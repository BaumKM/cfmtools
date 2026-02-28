use serde_json::{Value, json};

use crate::{
    config_spaces::Configuration,
    model::{
        cfm::CFM,
        feature::{Feature, FeatureVec},
    },
    utils::{
        data_structures::{
            DfsVisitor, Index, IndexTree, IndexTreeError, IndexVec, Tree, TreeHeights,
            TreeTraversal,
        },
        sorting::BucketSortByKey,
    },
};
use std::{
    collections::HashMap,
    fmt::{self, Write},
};

mod builder;

pub use builder::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
struct InstanceId(usize);

impl InstanceId {
    #[inline]
    pub fn new(u: usize) -> Self {
        InstanceId(u)
    }
}

impl Index for InstanceId {
    #[inline]
    fn to_usize(self) -> usize {
        self.0
    }

    #[inline]
    fn from_usize(u: usize) -> Self {
        Self(u)
    }
}

#[derive(Copy, Debug, Clone, Hash, PartialEq, Eq)]
pub struct FeatureInstance {
    feature: Feature,
    /// 1-based index of this instance among its feature's instances
    instance_number: usize,
}

impl FeatureInstance {
    #[must_use]
    pub fn new(feature: Feature, instance_number: usize) -> Self {
        Self {
            feature,
            instance_number,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstanceBasedConfiguration {
    configuration_tree: IndexTree<InstanceId>,
    /// mapping of instance ids to feature instances
    instances: IndexVec<InstanceId, FeatureInstance>,
    feature_counts: FeatureVec<usize>,
    // contains the start instance id for feature instances
    feature_offsets: FeatureVec<usize>,
}

impl InstanceBasedConfiguration {
    pub fn try_new(
        num_features: usize,
        root: FeatureInstance,
        parents: &HashMap<FeatureInstance, Option<FeatureInstance>>,
    ) -> Result<Self, InstanceBasedConfigError> {
        let mut feature_counts: FeatureVec<usize> = vec![0; num_features].into();

        for child in parents.keys() {
            feature_counts[child.feature] += 1;
        }

        // Compute feature_offsets
        let mut feature_offsets: FeatureVec<usize> = vec![0; num_features].into();

        let mut acc = 0;
        for (f, count) in feature_counts.enumerate() {
            feature_offsets[f] = acc;
            acc += count;
        }
        let total_instances = acc;

        //compute instance table
        let mut instances: IndexVec<InstanceId, FeatureInstance> = vec![
            FeatureInstance {
                feature: Feature::new(0),
                instance_number: 0,
            };
            total_instances
        ]
        .into();

        // Helper: convert FeatureInstance → InstanceId
        let id_of = |fi: FeatureInstance, feature_offsets: &FeatureVec<usize>| {
            let base = feature_offsets[fi.feature];
            let offset = fi.instance_number - 1;
            InstanceId::new(base + offset)
        };

        // Insert root
        let root_id = id_of(root, &feature_offsets);
        instances[root_id] = root;

        // Insert all instances appearing in parents_map
        for (&child, &parent) in parents {
            let cid = id_of(child, &feature_offsets);
            instances[cid] = child;

            if let Some(p) = parent {
                let pid = id_of(p, &feature_offsets);
                instances[pid] = p;
            }
        }

        // Build parents vector in id space
        let mut parents_id: IndexVec<InstanceId, Option<InstanceId>> =
            vec![None; total_instances].into();

        // Root must have no parent
        parents_id[root_id] = None;

        for (&child, &parent) in parents {
            let cid = id_of(child, &feature_offsets);
            let pid = parent.map(|p| id_of(p, &feature_offsets));
            parents_id[cid] = pid;
        }

        let tree = IndexTree::try_new(root_id, parents_id)
            .map_err(|err| InstanceBasedConfigError::Tree(err.map(|id| instances[id])))?;

        Ok(Self {
            configuration_tree: tree,
            instances,
            feature_counts,
            feature_offsets,
        })
    }

    #[allow(unused)]
    fn feature_instance(&self, id: InstanceId) -> &FeatureInstance {
        &self.instances[id]
    }

    fn id_of(&self, node: &FeatureInstance) -> InstanceId {
        let base = self.feature_offsets[node.feature];
        let offset = node.instance_number - 1;
        InstanceId::new(base + offset)
    }

    fn fmt_subtree(
        &self,
        feature_instance: &FeatureInstance,
        prefix: &mut String,
        is_last: bool,
        is_root: bool,
        model: &CFM,
        out: &mut String,
    ) {
        let feature_name = model.feature_name(&feature_instance.feature);

        let connector = if is_root {
            ""
        } else if is_last {
            "└── "
        } else {
            "├── "
        };

        let label = format!(
            "{}_{}",
            feature_name.name(),
            feature_instance.instance_number
        );

        writeln!(out, "{prefix}{connector}{label}").unwrap();
        let mut children = self.children(feature_instance).peekable();

        let base_len = prefix.len();

        if !is_root {
            if is_last {
                prefix.push_str("    ");
            } else {
                prefix.push_str("│   ");
            }
        }

        while let Some(child) = children.next() {
            let is_last_child = children.peek().is_none();
            self.fmt_subtree(child, prefix, is_last_child, false, model, out);
        }

        // Restore prefix
        prefix.truncate(base_len);
    }

    fn subtree_to_json(&self, feature_instance: &FeatureInstance, model: &CFM) -> Value {
        let f_name = &model.feature_name(&feature_instance.feature);

        json!({
            "name": format!("{}_{}", f_name.name(), feature_instance.instance_number),
            "feature": f_name.name(),
            "instance": feature_instance.instance_number,
            "children": self.children(feature_instance)
                .map(|c| self.subtree_to_json(c, model))
                .collect::<Vec<_>>()
        })
    }

    fn parent(&self, node: &FeatureInstance) -> Option<&FeatureInstance> {
        let feature_id = self.id_of(node);

        self.configuration_tree
            .parent(&feature_id)
            .map(|pid| &self.instances[pid])
    }
}

impl Tree for InstanceBasedConfiguration {
    type Node = FeatureInstance;
    type Children<'a>
        = InstanceBasedChildrenIter<'a>
    where
        Self: 'a;

    fn size(&self) -> usize {
        self.configuration_tree.size()
    }

    fn root(&self) -> &Self::Node {
        let root_id = self.configuration_tree.root();
        &self.instances[root_id]
    }

    fn children<'a>(&'a self, node: &'a Self::Node) -> Self::Children<'a> {
        let id = self.id_of(node);

        InstanceBasedChildrenIter {
            inner: self.configuration_tree.child_ids(id).iter(),
            instances: &self.instances,
        }
    }

    fn is_leaf(&self, node: &Self::Node) -> bool {
        let feature_id = self.id_of(node);
        self.configuration_tree.is_leaf(&feature_id)
    }
}

impl Configuration for InstanceBasedConfiguration {
    fn feature_counts(&self, _model: &CFM) -> &FeatureVec<usize> {
        &self.feature_counts
    }

    fn pretty_print(&self, model: &CFM) -> String {
        let mut out = String::new();
        let mut prefix = String::new();
        self.fmt_subtree(self.root(), &mut prefix, true, true, model, &mut out);
        out
    }

    fn serialize(&self, model: &CFM) -> Value {
        self.subtree_to_json(self.root(), model)
    }
}

pub struct InstanceBasedChildrenIter<'a> {
    inner: std::slice::Iter<'a, InstanceId>,
    instances: &'a IndexVec<InstanceId, FeatureInstance>,
}

impl<'a> Iterator for InstanceBasedChildrenIter<'a> {
    type Item = &'a FeatureInstance;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|id| &self.instances[*id])
    }
}
impl ExactSizeIterator for InstanceBasedChildrenIter<'_> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl InstanceBasedConfiguration {
    /// Computes the instance order of the configuration.
    /// Returns a vector mapping `InstanceId` -> rank (smaller = smaller in instance order).
    fn compute_instance_order(&self) -> IndexVec<InstanceId, usize> {
        let size = self.size();
        let depths = self.depths();
        let mut rank_of: IndexVec<InstanceId, usize> = vec![0; size].into();

        // nodes_by_depth[d] = all feature instances at depth d
        let mut nodes_by_depth: Vec<Vec<FeatureInstance>> = Vec::new();
        for (feature_instance, height) in depths {
            if nodes_by_depth.len() <= height {
                nodes_by_depth.resize(height + 1, Vec::new());
            }
            nodes_by_depth[height].push(feature_instance);
        }
        for layer in nodes_by_depth.into_iter().rev() {
            // Group by feature
            let mut feature_groups: HashMap<Feature, Vec<InstanceId>> = HashMap::new();
            for feature_instance in layer {
                feature_groups
                    .entry(feature_instance.feature)
                    .or_default()
                    .push(self.id_of(&feature_instance));
            }

            for (feature, nodes) in feature_groups {
                // Sort child ranks and compare lexicographically
                let mut nodes_with_keys: Vec<(InstanceId, Vec<usize>)> = nodes
                    .into_iter()
                    .map(|id| {
                        let mut child_ranks: Vec<usize> = self
                            .configuration_tree
                            .child_ids(id)
                            .iter()
                            .map(|cid| rank_of[*cid])
                            .collect();
                        child_ranks.sort_unstable();
                        (id, child_ranks)
                    })
                    .collect();

                // Sort lexicographically by the key
                nodes_with_keys.sort_unstable_by(|a, b| a.1.cmp(&b.1));

                // Start of the rank is determined by the feature
                let base = self.feature_offsets[feature];

                let mut current_rank = base;
                let mut prev_key: Option<&Vec<usize>> = None;

                for (id, key) in &nodes_with_keys {
                    // Only advance rank if the structural key changed
                    if prev_key.is_some_and(|prev| prev != key) {
                        current_rank += 1;
                    }
                    prev_key = Some(key);
                    rank_of[id] = current_rank;
                }
            }
        }

        rank_of
    }

    /// Returns a canonicalized version of this configuration.
    ///
    /// Canonicalization:
    /// - child instances are visited in increasing instance order,
    /// - instance numbers are renumbered per feature in DFS order,
    #[must_use]
    pub fn canonicalize(&self) -> CanonicalConfiguration {
        // ---------------------------------------------
        // Step 1: compute instance order
        // ---------------------------------------------
        let instance_order = self.compute_instance_order();

        // ---------------------------------------------
        // Step 2: DFS traversal with children sorted by instance order
        // ---------------------------------------------

        // contains for each feature how many instances have already been assigned
        let mut next_indices: FeatureVec<usize> = vec![0; self.feature_offsets.len()].into();

        // Mapping old instance -> new instance
        let mut rename: HashMap<FeatureInstance, FeatureInstance> = HashMap::new();

        // New parent map in FeatureInstance space
        let mut new_parents: HashMap<FeatureInstance, Option<FeatureInstance>> = HashMap::new();

        /// Visitor that performs the renaming during DFS
        struct RenameVisitor<'a> {
            configuration: &'a InstanceBasedConfiguration,
            next_indices: &'a mut FeatureVec<usize>,
            rename: &'a mut HashMap<FeatureInstance, FeatureInstance>,
            new_parents: &'a mut HashMap<FeatureInstance, Option<FeatureInstance>>,
        }

        impl DfsVisitor<IndexTree<InstanceId>> for RenameVisitor<'_> {
            fn enter(&mut self, current_id: &InstanceId) {
                let old_feature_instance = self.configuration.instances[*current_id];
                let f = old_feature_instance.feature;
                self.next_indices[f] += 1;

                let new_feature_instance = FeatureInstance {
                    feature: f,
                    instance_number: self.next_indices[f],
                };

                self.rename
                    .insert(old_feature_instance, new_feature_instance);
            }

            // when we exit a feature we have already renamed the feature itself and its parent
            fn exit(&mut self, child_id: &InstanceId) {
                let old_child = &self.configuration.instances[*child_id];

                if let Some(old_parent) = self.configuration.parent(old_child) {
                    let new_child = self.rename[old_child];
                    let new_parent = self.rename[old_parent];
                    self.new_parents.insert(new_child, Some(new_parent));
                }
            }
        }

        // Run ordered DFS
        let mut visitor = RenameVisitor {
            configuration: self,
            next_indices: &mut next_indices,
            rename: &mut rename,
            new_parents: &mut new_parents,
        };

        self.configuration_tree
            .run_dfs_ordered(&mut visitor, |node, tree| {
                let mut children: Vec<&InstanceId> = tree.children(node).collect();
                children.sort_by_key_bucket(|&&cid| instance_order[cid]);
                children
            });

        // Root has no parent
        let new_root = rename[self.root()];
        new_parents.insert(new_root, None);

        let canonical_configuration =
            Self::try_new(self.feature_offsets.len(), new_root, &new_parents)
                .expect("canonicalization must produce a valid configuration");

        CanonicalConfiguration {
            inner: canonical_configuration,
        }
    }
}

#[derive(Clone, Debug)]
pub enum InstanceBasedConfigError {
    MissingRoot(FeatureInstance),
    ParentMissingNode {
        child: FeatureInstance,
        parent: FeatureInstance,
    },
    Tree(IndexTreeError<FeatureInstance>),
}

impl fmt::Display for InstanceBasedConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRoot(root) => {
                write!(f, "missing root instance: {root:?}")
            }

            Self::ParentMissingNode { child, parent } => {
                write!(
                    f,
                    "parent {parent:?} of child {child:?} does not exist in configuration"
                )
            }

            Self::Tree(err) => {
                write!(f, "invalid configuration tree: {err}")
            }
        }
    }
}

impl std::error::Error for InstanceBasedConfigError {}

#[derive(Debug, Clone)]
pub struct CanonicalConfiguration {
    inner: InstanceBasedConfiguration,
}

impl Configuration for CanonicalConfiguration {
    fn feature_counts(&self, model: &CFM) -> &FeatureVec<usize> {
        self.inner.feature_counts(model)
    }

    fn pretty_print(&self, model: &CFM) -> String {
        self.inner.pretty_print(model)
    }

    fn serialize(&self, model: &CFM) -> Value {
        self.inner.serialize(model)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        config_spaces::instance::InstanceBasedConfigurationBuilder,
        model::cfm::cfm_test_helpers::CfmHelper,
    };

    use super::*;
    #[test]
    fn pretty_print_and_serialize() {
        //
        // Tree we build:
        //
        // Root_1
        // ├── A_1
        // │   ├── A1_1
        // │   └── A2_1
        // └── B_1
        //

        let num_features = 5;

        // Feature instances
        let root = FeatureInstance {
            feature: Feature::new(0), // Root
            instance_number: 1,
        };

        let a = FeatureInstance {
            feature: Feature::new(1), // A
            instance_number: 1,
        };

        let a1 = FeatureInstance {
            feature: Feature::new(2), // A1
            instance_number: 1,
        };

        let a2 = FeatureInstance {
            feature: Feature::new(3), // A2
            instance_number: 1,
        };

        let b = FeatureInstance {
            feature: Feature::new(4), // B
            instance_number: 1,
        };

        // Parents (FeatureInstance → Option<FeatureInstance>)
        let mut parents = HashMap::new();

        parents.insert(root, None);
        parents.insert(a, Some(root));
        parents.insert(a1, Some(a));
        parents.insert(a2, Some(a));
        parents.insert(b, Some(root));

        let config = InstanceBasedConfiguration::try_new(num_features, root, &parents)
            .expect("tree must be valid");

        // Mock model
        let model = CfmHelper::from_names_and_parents(
            vec!["Root", "A", "A1", "A2", "B"],
            vec![("A", "Root"), ("B", "Root"), ("A1", "A"), ("A2", "A")],
        );

        let pretty = config.pretty_print(&model);
        let expected = "Root_1\n├── A_1\n│   ├── A1_1\n│   └── A2_1\n└── B_1\n";

        assert_eq!(pretty, expected);

        let json = config.serialize(&model);

        assert_eq!(json["name"], "Root_1");
        assert_eq!(json["children"].as_array().unwrap().len(), 2);

        let a_json = &json["children"][0];
        assert_eq!(a_json["name"], "A_1");
        assert_eq!(a_json["children"].as_array().unwrap().len(), 2);

        let b_json = &json["children"][1];
        assert_eq!(b_json["name"], "B_1");
        assert!(b_json["children"].as_array().unwrap().is_empty());
    }

    struct FeatureInstanceFactory {
        /// Maps feature name -> feature index
        feature_ids: HashMap<String, usize>,

        /// Maps feature name -> next instance number
        next_index: HashMap<String, usize>,
    }
    impl FeatureInstanceFactory {
        fn new() -> Self {
            Self {
                feature_ids: HashMap::new(),
                next_index: HashMap::new(),
            }
        }

        fn fi(&mut self, feature: &str) -> FeatureInstance {
            let next_feature_id = self.feature_ids.len();
            let fid = *self
                .feature_ids
                .entry(feature.to_string())
                .or_insert(next_feature_id);

            let next = self.next_index.entry(feature.to_string()).or_insert(1);

            let instance_number = *next;
            *next += 1;

            FeatureInstance::new(Feature::new(fid), instance_number)
        }
    }

    #[test]
    fn canonicalize_sibling_consistency() {
        // ------------------------------------------------------------
        // Original (non-canonical):
        //
        //   Root_1
        //   ├── A_2
        //   └── A_1
        //       └── B_1
        //
        // A_1 > A_2 in instance order (non-leaf > leaf), but the indices
        // are reversed, violating sibling consistency.
        //
        // Expected canonical:
        //
        //   Root_1
        //   ├── A_1        (leaf)
        //   └── A_2        (has child)
        //       └── B_1
        //
        // ------------------------------------------------------------
        let model = CfmHelper::from_names_and_parents(
            vec!["Root", "A", "B"],
            vec![("A", "Root"), ("B", "A")],
        );

        let mut factory = FeatureInstanceFactory::new();

        let root = factory.fi("root");
        let a1 = factory.fi("a");
        let a2 = factory.fi("a");
        let b1 = factory.fi("b");

        // Build non-canonical input
        let mut builder = InstanceBasedConfigurationBuilder::new(3);
        builder.set_root(root);
        builder.set_parent(a2, root).unwrap();
        builder.set_parent(a1, root).unwrap();
        builder.set_parent(b1, a1).unwrap();

        let input = builder.build().unwrap();
        let canon = input.canonicalize();

        // Build expected canonical
        let mut builder = InstanceBasedConfigurationBuilder::new(3);
        builder.set_root(root);
        builder.set_parent(a1, root).unwrap();
        builder.set_parent(a2, root).unwrap();
        builder.set_parent(b1, a2).unwrap();

        let expected = builder.build().unwrap();

        assert_eq!(canon.serialize(&model), expected.serialize(&model),);
    }

    #[test]
    fn canonicalize_subtree_monotonicity() {
        // ------------------------------------------------------------
        // Original (non-canonical):
        //
        //   Root_1
        //   ├── A_1
        //   │   └── B_2
        //   └── A_2
        //       └── B_1
        //
        // B indices are swapped across A subtrees:
        //   B_1 is under A_2, but B_2 is under A_1.
        //   This violates subtree index monotonicity.
        //
        // Expected canonical:
        //
        //   Root_1
        //   ├── A_1
        //   │   └── B_1
        //   └── A_2
        //       └── B_2
        //
        // ------------------------------------------------------------
        let model = CfmHelper::from_names_and_parents(
            vec!["Root", "A", "B"],
            vec![("A", "Root"), ("B", "A")],
        );

        let mut factory = FeatureInstanceFactory::new();

        let root = factory.fi("root");
        let a1 = factory.fi("a");
        let a2 = factory.fi("a");
        let b1 = factory.fi("b");
        let b2 = factory.fi("b");

        // Build non-canonical input
        let mut builder = InstanceBasedConfigurationBuilder::new(3);
        builder.set_root(root);
        builder.set_parent(a1, root).unwrap();
        builder.set_parent(a2, root).unwrap();

        builder.set_parent(b2, a1).unwrap();
        builder.set_parent(b1, a2).unwrap();

        let input = builder.build().unwrap();
        let canon = input.canonicalize();

        // Build expected canonical
        let mut builder = InstanceBasedConfigurationBuilder::new(3);
        builder.set_root(root);

        builder.set_parent(a1, root).unwrap();
        builder.set_parent(a2, root).unwrap();
        builder.set_parent(b1, a1).unwrap();
        builder.set_parent(b2, a2).unwrap();

        let expected = builder.build().unwrap();

        assert_eq!(canon.serialize(&model), expected.serialize(&model));
    }

    #[test]
    fn canonicalize_sibling_consistency_and_subtree_monotonicity() {
        // ------------------------------------------------------------
        // Original (non-canonical):
        //
        //   Root_1
        //   ├── A_2
        //   │   ├── B_1
        //   │   └── C_2
        //   └── A_1
        //       └── C_1
        //
        // Expected canonical:
        //
        //   Root_1
        //   ├── A_1
        //   │   ├── B_1
        //   │   └── C_1
        //   └── A_2
        //       └── C_2
        //
        // ------------------------------------------------------------
        let model = CfmHelper::from_names_and_parents(
            vec!["Root", "A", "B", "C"],
            vec![("A", "Root"), ("B", "A"), ("C", "A")],
        );

        let mut factory = FeatureInstanceFactory::new();

        let root = factory.fi("root");
        let a1 = factory.fi("a");
        let a2 = factory.fi("a");
        let b1 = factory.fi("b");
        let c1 = factory.fi("c");
        let c2 = factory.fi("c");

        // Build non-canonical input
        let mut builder = InstanceBasedConfigurationBuilder::new(4);
        builder.set_root(root);

        builder.set_parent(a2, root).unwrap();
        builder.set_parent(a1, root).unwrap();

        // A_2 subtree
        builder.set_parent(b1, a2).unwrap();
        builder.set_parent(c2, a2).unwrap();

        // A_1 subtree
        builder.set_parent(c1, a1).unwrap();

        let input = builder.build().unwrap();
        let canon = input.canonicalize();

        // Build expected canonical
        let mut builder = InstanceBasedConfigurationBuilder::new(4);
        builder.set_root(root);

        builder.set_parent(a1, root).unwrap();
        builder.set_parent(a2, root).unwrap();

        builder.set_parent(b1, a1).unwrap();
        builder.set_parent(c1, a1).unwrap();
        builder.set_parent(c2, a2).unwrap();

        let expected = builder.build().unwrap();
        assert_eq!(canon.serialize(&model), expected.serialize(&model));
    }

    #[test]
    fn canonicalize_complex_configuration() {
        // --------------------------------------------------------------------
        // Original (non-canonical):
        //
        //   Root_1
        //   ├── A_3
        //   │   ├── B_2
        //   │   │   └── D_1
        //   │   └── C_1
        //   ├── A_1
        //   │   └── B_1
        //   │       └── D_2
        //   └── A_2
        //       ├── C_2
        //       └── B_3
        //
        //
        // Expected canonical:
        //
        //   Root_1
        //   ├── A_1
        //   │   ├── B_1
        //   │   └── C_1
        //   ├── A_2
        //   │   └── B_2
        //   │       └── D_1
        //   └── A_3
        //       ├── B_3
        //       │   └── D_2
        //       └── C_2
        //
        //
        // --------------------------------------------------------------------

        let model = CfmHelper::from_names_and_parents(
            vec!["Root", "A", "B", "C", "D"],
            vec![("A", "Root"), ("B", "A"), ("C", "A"), ("D", "B")],
        );

        let mut factory = FeatureInstanceFactory::new();

        // Instances
        let root = factory.fi("root");

        let a1 = factory.fi("a");
        let a2 = factory.fi("a");
        let a3 = factory.fi("a");

        let b1 = factory.fi("b");
        let b2 = factory.fi("b");
        let b3 = factory.fi("b");

        let c1 = factory.fi("c");
        let c2 = factory.fi("c");

        let d1 = factory.fi("d");
        let d2 = factory.fi("d");

        // Build non-canonical input
        let mut builder = InstanceBasedConfigurationBuilder::new(5);
        builder.set_root(root);

        // A children in wrong order
        builder.set_parent(a3, root).unwrap();
        builder.set_parent(a1, root).unwrap();
        builder.set_parent(a2, root).unwrap();

        // A_3 subtree
        builder.set_parent(b2, a3).unwrap();
        builder.set_parent(c1, a3).unwrap();
        builder.set_parent(d1, b2).unwrap();

        // A_1 subtree
        builder.set_parent(b1, a1).unwrap();
        builder.set_parent(d2, b1).unwrap();

        // A_2 subtree
        builder.set_parent(c2, a2).unwrap();
        builder.set_parent(b3, a2).unwrap();

        let input = builder.build().unwrap();
        let canon = input.canonicalize();

        // Build expected canonical configuration
        let mut builder = InstanceBasedConfigurationBuilder::new(5);
        builder.set_root(root);

        // Canonical A ordering
        builder.set_parent(a1, root).unwrap();
        builder.set_parent(a2, root).unwrap();
        builder.set_parent(a3, root).unwrap();

        // A_1 subtree
        builder.set_parent(b1, a1).unwrap();
        builder.set_parent(c1, a1).unwrap();

        // A_2 subtree
        builder.set_parent(b2, a2).unwrap();
        builder.set_parent(d1, b2).unwrap();

        // A_3 subtree
        builder.set_parent(b3, a3).unwrap();
        builder.set_parent(d2, b3).unwrap();
        builder.set_parent(c2, a3).unwrap();

        let expected = builder.build().unwrap();
        print!("{}", canon.pretty_print(&model));
        println!("{}", expected.pretty_print(&model));
        assert_eq!(canon.serialize(&model), expected.serialize(&model));
    }

    #[test]
    fn canonicalize_very_large_configuration() {
        // --------------------------------------------------------------------
        // Original (non-canonical):
        //
        //   Root_1
        //   ├── A_3
        //   │   ├── B_2
        //   │   │   └── D_2
        //   │   └── C_3
        //   │       └── E_3
        //   ├── A_1
        //   │   ├── C_1
        //   │   │   └── E_1
        //   │   └── B_1
        //   ├── A_4
        //   │   └── B_4
        //   │       └── D_3
        //   └── A_2
        //       ├── C_2
        //       │   └── E_2
        //       └── B_3
        //           └── D_1
        //
        //
        // Expected canonical:
        //
        //   Root_1
        //   ├── A_1
        //   │   ├── B_1
        //   │   └── C_1
        //   │       └── E_1
        //   ├── A_2
        //   │   └── B_2
        //   │       └── D_1
        //   ├── A_3
        //   │   ├── B_3
        //   │   │   └── D_2
        //   │   └── C_2
        //   │       └── E_2
        //   └── A_4
        //       ├── B_4
        //       │   └── D_3
        //       └── C_3
        //           └── E_3

        //
        //
        // --------------------------------------------------------------------

        let model = CfmHelper::from_names_and_parents(
            vec!["Root", "A", "B", "C", "D", "E"],
            vec![
                ("A", "Root"),
                ("B", "A"),
                ("C", "A"),
                ("D", "B"),
                ("E", "C"),
            ],
        );

        let mut factory = FeatureInstanceFactory::new();

        let root = factory.fi("root");

        let a1 = factory.fi("a");
        let a2 = factory.fi("a");
        let a3 = factory.fi("a");
        let a4 = factory.fi("a");

        let b1 = factory.fi("b");
        let b2 = factory.fi("b");
        let b3 = factory.fi("b");
        let b4 = factory.fi("b");

        let c1 = factory.fi("c");
        let c2 = factory.fi("c");
        let c3 = factory.fi("c");

        let d1 = factory.fi("d");
        let d2 = factory.fi("d");
        let d3 = factory.fi("d");

        let e1 = factory.fi("e");
        let e2 = factory.fi("e");
        let e3 = factory.fi("e");

        // Build non-canonical input
        let mut builder = InstanceBasedConfigurationBuilder::new(6);
        builder.set_root(root);

        builder.set_parent(a3, root).unwrap();
        builder.set_parent(a1, root).unwrap();
        builder.set_parent(a4, root).unwrap();
        builder.set_parent(a2, root).unwrap();

        // A_3 subtree
        builder.set_parent(b2, a3).unwrap();
        builder.set_parent(c3, a3).unwrap();
        builder.set_parent(d2, b2).unwrap();
        builder.set_parent(e3, c3).unwrap();

        // A_1 subtree
        builder.set_parent(c1, a1).unwrap();
        builder.set_parent(b1, a1).unwrap();
        builder.set_parent(e1, c1).unwrap();

        // A_4 subtree
        builder.set_parent(b4, a4).unwrap();
        builder.set_parent(d3, b4).unwrap();

        // A_2 subtree
        builder.set_parent(c2, a2).unwrap();
        builder.set_parent(b3, a2).unwrap();
        builder.set_parent(e2, c2).unwrap();
        builder.set_parent(d1, b3).unwrap();

        let input = builder.build().unwrap();
        let canon = input.canonicalize();

        // Build expected canonical configuration
        let mut builder = InstanceBasedConfigurationBuilder::new(6);
        builder.set_root(root);

        builder.set_parent(a1, root).unwrap();
        builder.set_parent(a2, root).unwrap();
        builder.set_parent(a3, root).unwrap();
        builder.set_parent(a4, root).unwrap();

        // A_1
        builder.set_parent(b1, a1).unwrap();
        builder.set_parent(c1, a1).unwrap();
        builder.set_parent(e1, c1).unwrap();

        // A_2
        builder.set_parent(b2, a2).unwrap();
        builder.set_parent(d1, b2).unwrap();

        // A_3
        builder.set_parent(b3, a3).unwrap();
        builder.set_parent(c2, a3).unwrap();
        builder.set_parent(d2, b3).unwrap();
        builder.set_parent(e2, c2).unwrap();

        // A_4
        builder.set_parent(b4, a4).unwrap();
        builder.set_parent(c3, a4).unwrap();
        builder.set_parent(d3, b4).unwrap();
        builder.set_parent(e3, c3).unwrap();

        let expected = builder.build().unwrap();

        assert_eq!(canon.serialize(&model), expected.serialize(&model));
    }
}

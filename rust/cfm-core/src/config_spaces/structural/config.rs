use std::sync::Arc;

use serde_json::Value;

use crate::{
    config_spaces::{
        Configuration,
        instance::{CanonicalConfiguration, FeatureInstance, InstanceBasedConfigurationBuilder},
    },
    model::{
        cfm::CFM,
        feature::{Feature, FeatureVec},
    },
    utils::data_structures::{DfsVisitor, Index, Tree},
};

mod builder;

pub use builder::*;

#[derive(Debug, Clone)]
pub struct StructuralConfiguration {
    root: Arc<StructuralNode>,
    feature_counts: FeatureVec<usize>,
    size: usize,
}

#[derive(Debug, Clone)]
pub struct StructuralNode {
    id: NodeId,
    feature: Feature,
    children: Vec<ChildEntry>,
}

impl StructuralNode {
    #[must_use]
    pub fn feature(&self) -> &Feature {
        &self.feature
    }
    #[must_use]
    pub fn children(&self) -> &[ChildEntry] {
        &self.children
    }
}

#[derive(Debug, Clone)]
pub struct ChildEntry {
    node: Arc<StructuralNode>,
    multiplicity: usize,
}

impl ChildEntry {
    #[must_use]
    pub fn multiplicity(&self) -> usize {
        self.multiplicity
    }
    #[must_use]
    pub fn node(&self) -> &Arc<StructuralNode> {
        &self.node
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId {
    id: usize,
}

impl Index for NodeId {
    fn to_usize(self) -> usize {
        self.id
    }

    fn from_usize(u: usize) -> Self {
        Self { id: u }
    }
}

impl Configuration for StructuralConfiguration {
    fn feature_counts(&self, _model: &CFM) -> &FeatureVec<usize> {
        &self.feature_counts
    }

    fn pretty_print(&self, model: &CFM) -> String {
        self.canonical_representation(model).pretty_print(model)
    }

    fn serialize(&self, model: &CFM) -> Value {
        self.canonical_representation(model).serialize(model)
    }
}
impl StructuralConfiguration {
    #[must_use]
    pub fn build_canonical_repr(&self, _model: &CFM) -> CanonicalConfiguration {
        use crate::utils::data_structures::TreeTraversal;

        let num_features = self.feature_counts.len();
        let mut builder = InstanceBasedConfigurationBuilder::new(num_features);

        struct Visitor<'a> {
            builder: &'a mut InstanceBasedConfigurationBuilder,

            /// Stack of instance vectors for the current DFS path.
            instance_stack: Vec<Vec<FeatureInstance>>,

            /// Per-feature next instance number (1-based).
            next_number: FeatureVec<usize>,
        }

        impl DfsVisitor<StructuralConfiguration> for Visitor<'_> {
            fn enter(&mut self, node: &StructuralNode) {
                let feature = node.feature;
                let instance_number = self.next_number[feature] + 1;
                self.next_number[feature] = instance_number;

                let instance = FeatureInstance::new(feature, instance_number);

                // Attach to parent if any, otherwise this is the root.
                if let Some(parent_instances) = self.instance_stack.last() {
                    // By construction the last instance of the parent stack is the parent node.
                    let parent = *parent_instances
                        .last()
                        .expect("parent instance stack is empty");

                    self.builder
                        .set_parent(instance, parent)
                        .expect("invalid parent relation");
                } else {
                    self.builder.set_root(instance);
                }

                // Push this instance as the current subtree instance set.
                self.instance_stack.push(vec![instance]);
            }

            fn exit(&mut self, _node: &StructuralNode) {
                self.instance_stack.pop().expect("instance stack underflow");
            }
        }

        let mut visitor = Visitor {
            builder: &mut builder,
            instance_stack: Vec::new(),
            next_number: vec![0; num_features].into(),
        };

        // Run DFS over the structural tree.
        self.run_dfs_ordered(&mut visitor, |node, _tree| {
            let mut expanded: Vec<&StructuralNode> = Vec::new();

            //inflate for every node the number of children
            for entry in &node.children {
                for _ in 0..entry.multiplicity {
                    expanded.push(entry.node.as_ref());
                }
            }

            expanded
        });

        // Build instance-based configuration and canonicalize it.
        let instance_based = builder
            .build()
            .expect("structural configuration has always valid instance based repr");

        instance_based.canonicalize()
    }

    #[must_use]
    pub fn canonical_representation(&self, model: &CFM) -> CanonicalConfiguration {
        self.build_canonical_repr(model)
    }
}

impl Tree for StructuralConfiguration {
    type Node = StructuralNode;
    type Children<'a>
        = StructuralChildrenIter<'a>
    where
        Self: 'a;

    fn size(&self) -> usize {
        self.size
    }

    fn root(&self) -> &Self::Node {
        &self.root
    }

    fn children<'a>(&'a self, node: &'a Self::Node) -> Self::Children<'a> {
        StructuralChildrenIter {
            inner: node.children.iter(),
        }
    }

    fn is_leaf(&self, node: &Self::Node) -> bool {
        node.children.is_empty()
    }
}

pub struct StructuralChildrenIter<'a> {
    inner: std::slice::Iter<'a, ChildEntry>,
}

impl<'a> Iterator for StructuralChildrenIter<'a> {
    type Item = &'a StructuralNode;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|entry| entry.node.as_ref())
    }
}

impl ExactSizeIterator for StructuralChildrenIter<'_> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

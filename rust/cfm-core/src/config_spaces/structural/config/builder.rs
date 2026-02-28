use std::{collections::HashMap, sync::Arc};

use crate::{
    config_spaces::structural::{
        StructuralConfiguration,
        config::{ChildEntry, NodeId, StructuralNode},
    },
    model::feature::{Feature, FeatureVec},
    utils::data_structures::Index,
};

#[derive(Clone, Hash, PartialEq, Eq)]
struct NodeKey {
    feature: Feature,
    children: Vec<NodeKeyChildEntry>,
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct NodeKeyChildEntry {
    node_id: NodeId,
    multiplicity: usize,
}

#[derive(Clone)]
struct AggregateValues {
    size: usize,
    feature_counts: FeatureVec<usize>,
}

pub struct StructuralBuilder {
    next_id: usize,
    num_features: usize,
    table: HashMap<Arc<NodeKey>, Arc<StructuralNode>>,
    aggregate_values: HashMap<NodeId, AggregateValues>,
    insert_log: Vec<(Arc<NodeKey>, NodeId)>,
}
impl StructuralBuilder {
    #[must_use]
    pub fn new(num_features: usize) -> Self {
        Self {
            next_id: 0,
            num_features,
            table: HashMap::new(),
            aggregate_values: HashMap::new(),
            insert_log: Vec::new(),
        }
    }
    /// Start building a node.
    pub fn begin_node(&mut self, feature: &Feature) -> NodeBuilder {
        NodeBuilder {
            feature: *feature,
            children: Vec::new(),
        }
    }
    /// Finish a node and return the interned node.
    pub fn finish_node(&mut self, node: NodeBuilder) -> Arc<StructuralNode> {
        self.intern_node(node.feature, node.children)
    }

    /// Finalize into a configuration.
    #[must_use]
    pub fn finish(mut self, root: Arc<StructuralNode>) -> StructuralConfiguration {
        let aggregate_values = self
            .aggregate_values
            .remove(&root.id)
            .expect("root aggregate missing");

        StructuralConfiguration {
            root,
            feature_counts: aggregate_values.feature_counts,
            size: aggregate_values.size,
        }
    }

    fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        NodeId::from_usize(id)
    }

    /// Internal: intern + compute aggregate.
    fn intern_node(
        &mut self,
        feature: Feature,
        mut children: Vec<(Arc<StructuralNode>, usize)>,
    ) -> Arc<StructuralNode> {
        // normalize
        children.retain(|(_, multiplicity)| *multiplicity > 0);
        children.sort_unstable_by_key(|(g, _)| g.id.to_usize());
        // Merge identical node ids
        let mut write = 0;
        for read in 0..children.len() {
            if write == 0 || children[read].0.id != children[write - 1].0.id {
                // New node
                children[write] = children[read].clone();
                write += 1;
            } else {
                // Same node → accumulate multiplicity
                children[write - 1].1 += children[read].1;
            }
        }
        children.truncate(write);

        // build interning key
        let key = Arc::new(NodeKey {
            feature,
            children: children
                .iter()
                .map(|(g, m)| NodeKeyChildEntry {
                    node_id: g.id,
                    multiplicity: *m,
                })
                .collect(),
        });

        // reuse existing node if present
        if let Some(existing) = self.table.get(&key) {
            return existing.clone();
        }

        // allocate new id
        let id = self.alloc_id();

        let child_entries: Vec<ChildEntry> = children
            .iter()
            .map(|(node, m)| ChildEntry {
                node: node.clone(),
                multiplicity: *m,
            })
            .collect();

        let node = Arc::new(StructuralNode {
            id,
            feature,
            children: child_entries,
        });

        // compute aggregates
        let mut feature_counts: FeatureVec<usize> = vec![0; self.num_features].into();
        feature_counts[feature] += 1;
        let mut size = 1usize;

        for (child, mult) in children {
            let child_agg = self
                .aggregate_values
                .get(&child.id)
                .expect("child aggregate must exist");

            size += mult * child_agg.size;

            // add m * child counts
            for (feature, count) in feature_counts.enumerate_mut() {
                *count += mult * child_agg.feature_counts[feature];
            }
        }

        self.aggregate_values.insert(
            id,
            AggregateValues {
                size,
                feature_counts,
            },
        );
        self.insert_log.push((key.clone(), id));
        self.table.insert(key, node.clone());

        node
    }
}

pub struct NodeBuilder {
    feature: Feature,
    children: Vec<(Arc<StructuralNode>, usize)>,
}

impl NodeBuilder {
    /// Add one child node with multiplicity.
    #[must_use]
    pub fn add_child(mut self, child: Arc<StructuralNode>, multiplicity: usize) -> Self {
        if multiplicity > 0 {
            self.children.push((child, multiplicity));
        }
        self
    }
    /// Add many children at once.
    #[must_use]
    pub fn add_children<I>(mut self, iter: I) -> Self
    where
        I: IntoIterator<Item = Arc<StructuralNode>>,
    {
        for child in iter {
            self.children.push((child, 1));
        }
        self
    }
}

impl StructuralBuilder {
    /// Create a rollback checkpoint.
    #[must_use]
    pub fn checkpoint(&self) -> BuilderCheckpoint {
        BuilderCheckpoint {
            next_id: self.next_id,
            insert_log_len: self.insert_log.len(),
        }
    }

    /// Roll back all allocations after a checkpoint.
    pub fn rollback(&mut self, cp: &BuilderCheckpoint) {
        self.next_id = cp.next_id;

        // Remove all inserts performed after the checkpoint
        while self.insert_log.len() > cp.insert_log_len {
            if let Some((key, id)) = self.insert_log.pop() {
                self.table.remove(&key);
                self.aggregate_values.remove(&id);
            }
        }
    }

    /// Count how many times each distinct structural node appears.
    ///
    /// The returned vector contains only the multiplicities.
    /// The order of elements in the vector is arbitrary.
    #[must_use]
    pub fn count_configurations(&self, nodes: &[Arc<StructuralNode>]) -> Vec<usize> {
        let mut counts: HashMap<*const StructuralNode, usize> = HashMap::new();

        for g in nodes {
            let ptr = Arc::as_ptr(g);
            *counts.entry(ptr).or_insert(0) += 1;
        }

        counts.into_values().collect()
    }
}

pub struct BuilderCheckpoint {
    next_id: usize,
    insert_log_len: usize,
}

use serde::Serialize;

use crate::utils::data_structures::Tree;

#[derive(Debug, Clone, Serialize)]
pub struct TreeSummary {
    pub size: usize,
    pub height: usize,
    pub leaves: usize,
    pub internal_nodes: usize,
    pub avg_internal_branching_factor: f64,
    pub max_branching_factor: usize,
}

pub trait TreeStatistics: Tree {
    fn tree_summary(&self) -> TreeSummary
    where
        Self: Sized,
    {
        use std::cmp::max;

        let root = self.root();

        let mut stack = vec![(root, 0usize)];

        let mut size = 0usize;
        let mut leaves = 0usize;
        let mut internal_nodes = 0usize;
        let mut height = 0usize;

        let mut total_children = 0usize;
        let mut max_branching_factor = 0usize;

        while let Some((node, depth)) = stack.pop() {
            size += 1;
            height = max(height, depth);

            let children: Vec<_> = self.children(node).collect();
            let degree = children.len();

            total_children += degree;
            max_branching_factor = max(max_branching_factor, degree);

            if degree == 0 {
                leaves += 1;
            } else {
                internal_nodes += 1;
            }

            for child in children {
                stack.push((child, depth + 1));
            }
        }

        let avg_branching_factor = if internal_nodes > 0 {
            total_children as f64 / internal_nodes as f64
        } else {
            0.0
        };

        TreeSummary {
            size,
            height,
            leaves,
            internal_nodes,
            avg_internal_branching_factor: avg_branching_factor,
            max_branching_factor,
        }
    }
}

impl<T: Tree> TreeStatistics for T {}

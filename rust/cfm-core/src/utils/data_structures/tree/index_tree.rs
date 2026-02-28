use std::{error::Error, fmt};

use crate::utils::data_structures::{Index, IndexVec, Tree};

#[derive(Debug, Clone)]
pub struct IndexTree<N: Index> {
    root: N,
    parents: IndexVec<N, Option<N>>,
    children: IndexVec<N, Vec<N>>,
}

impl<N: Index> IndexTree<N> {
    pub fn try_new(root: N, parents: IndexVec<N, Option<N>>) -> Result<Self, IndexTreeError<N>> {
        let n = parents.len();

        // ============================================================
        // 1) Check index range
        // ============================================================
        if root.to_usize() >= n {
            return Err(IndexTreeError::InvalidRoot { root, len: n });
        }

        // Parent indices must be valid
        for (node, parent) in parents.enumerate() {
            if let Some(parent) = parent
                && parent.to_usize() >= n
            {
                return Err(IndexTreeError::ParentOutOfRange {
                    parent: *parent,
                    node,
                    len: n,
                });
            }
        }

        // ============================================================
        // 2) Parent structure
        // ============================================================

        // Every non-root must have exactly one parent
        for (node, parent) in parents.enumerate() {
            if node == root {
                if parent.is_some() {
                    return Err(IndexTreeError::RootHasParent {
                        root: node,
                        parent: parent.unwrap(),
                    });
                }
            } else if parent.is_none() {
                return Err(IndexTreeError::MissingParent { node });
            }
        }

        // ============================================================
        // 3) Build child sets
        // ============================================================

        let mut children: IndexVec<N, Vec<N>> = vec![Vec::new(); n].into();

        for (child, parent) in parents.enumerate() {
            if let Some(p) = parent {
                children[p].push(child);
            }
        }

        // ============================================================
        // 4) Connectivity + tree structure
        // ============================================================
        let mut visited: IndexVec<N, bool> = vec![false; n].into();
        let mut stack = vec![root];

        while let Some(node) = stack.pop() {
            if visited[node] {
                // Cycle or multiple incoming edges
                return Err(IndexTreeError::NotATree);
            }
            visited[node] = true;

            for &child in &children[node] {
                stack.push(child);
            }
        }

        let unreachable: Vec<_> = parents
            .enumerate()
            .filter(|(n, _)| !visited[*n])
            .map(|(n, _)| n)
            .collect();

        if !unreachable.is_empty() {
            return Err(IndexTreeError::UnreachableNodes { unreachable });
        }

        Ok(Self {
            root,
            parents,
            children,
        })
    }

    pub fn child_ids(&self, node: N) -> &[N] {
        &self.children[node]
    }

    pub fn parent(&self, node: &N) -> Option<&N> {
        self.parents[node].as_ref()
    }
}

impl<N: Index> Tree for IndexTree<N> {
    type Node = N;
    type Children<'a>
        = std::slice::Iter<'a, N>
    where
        Self: 'a;

    fn size(&self) -> usize {
        self.parents.len()
    }

    fn root(&self) -> &Self::Node {
        &self.root
    }

    fn children<'a>(&'a self, node: &'a Self::Node) -> Self::Children<'a> {
        self.children[node].iter()
    }

    fn is_leaf(&self, node: &Self::Node) -> bool {
        self.children[node].is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexTreeError<N> {
    /// root out of range
    InvalidRoot { root: N, len: usize },

    /// Parent is out of range for node
    ParentOutOfRange { parent: N, node: N, len: usize },

    /// Root must not have a parent
    RootHasParent { root: N, parent: N },

    /// Non-root node has no parent
    MissingParent { node: N },

    /// Cycle or multiple-parent detected
    NotATree,

    /// Some nodes were not reachable from root
    UnreachableNodes { unreachable: Vec<N> },
}

impl<T> IndexTreeError<T> {
    pub fn map<F, M>(self, mut f: M) -> IndexTreeError<F>
    where
        M: FnMut(T) -> F,
    {
        match self {
            Self::InvalidRoot { root, len } => IndexTreeError::InvalidRoot { root: f(root), len },

            Self::ParentOutOfRange { parent, node, len } => IndexTreeError::ParentOutOfRange {
                parent: f(parent),
                node: f(node),
                len,
            },

            Self::RootHasParent { root, parent } => IndexTreeError::RootHasParent {
                root: f(root),
                parent: f(parent),
            },

            Self::MissingParent { node } => IndexTreeError::MissingParent { node: f(node) },

            Self::NotATree => IndexTreeError::NotATree,

            Self::UnreachableNodes { unreachable } => IndexTreeError::UnreachableNodes {
                unreachable: unreachable.into_iter().map(&mut f).collect(),
            },
        }
    }
}

impl<N: fmt::Debug> fmt::Display for IndexTreeError<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use IndexTreeError::{
            InvalidRoot, MissingParent, NotATree, ParentOutOfRange, RootHasParent, UnreachableNodes,
        };

        match self {
            InvalidRoot { root, len } => {
                write!(f, "root {root:?} is out of range (len = {len})")
            }
            ParentOutOfRange { parent, node, len } => {
                write!(
                    f,
                    "parent {parent:?} is out of range for node {node:?} (len = {len})"
                )
            }

            RootHasParent { root, parent } => {
                write!(f, "root {root:?} must not have parent (found {parent:?})")
            }
            MissingParent { node } => {
                write!(f, "node {node:?} is missing a parent")
            }

            NotATree => {
                write!(f, "graph is not a tree (cycle or multiple parents)")
            }
            UnreachableNodes { unreachable } => {
                write!(f, "unreachable nodes: {unreachable:?}")
            }
        }
    }
}

impl<N: fmt::Debug> Error for IndexTreeError<N> {}

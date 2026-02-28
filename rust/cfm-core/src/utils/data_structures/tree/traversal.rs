use std::{collections::HashMap, hash::Hash};

use crate::utils::data_structures::Tree;

enum DfsEvent<'a, N> {
    Enter(&'a N),
    Exit(&'a N),
}

struct Dfs<'a, T, F>
where
    T: Tree,
    F: for<'b> FnMut(&'b T::Node, &'b T) -> Vec<&'b T::Node>,
{
    tree: &'a T,
    child_fn: F,
    stack: Vec<Frame<'a, T>>,
}

struct Frame<'a, T: Tree> {
    node: &'a T::Node,
    children: Vec<&'a T::Node>,
    entered: bool,
    next_child: usize,
}

impl<'a, T, F> Dfs<'a, T, F>
where
    T: Tree,
    F: for<'b> FnMut(&'b T::Node, &'b T) -> Vec<&'b T::Node>,
{
    fn new(tree: &'a T, mut child_fn: F) -> Self {
        let root = tree.root();
        let children = child_fn(root, tree);

        Self {
            tree,
            child_fn,
            stack: vec![Frame {
                node: root,
                children,
                entered: false,
                next_child: 0,
            }],
        }
    }
}

impl<'a, T, F> Iterator for Dfs<'a, T, F>
where
    T: Tree,
    F: for<'b> FnMut(&'b T::Node, &'b T) -> Vec<&'b T::Node>,
{
    type Item = DfsEvent<'a, T::Node>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let frame = self.stack.last_mut()?;

            // First encounter of this node
            if !frame.entered {
                frame.entered = true;
                return Some(DfsEvent::Enter(frame.node));
            }

            // Descend into the next child if any remain
            if frame.next_child < frame.children.len() {
                let child = frame.children[frame.next_child];
                frame.next_child += 1;

                let grandchildren = (self.child_fn)(child, self.tree);

                self.stack.push(Frame {
                    node: child,
                    children: grandchildren,
                    entered: false,
                    next_child: 0,
                });
                continue;
            }

            // All children processed: exit this node
            let frame = self.stack.pop()?;
            return Some(DfsEvent::Exit(frame.node));
        }
    }
}

pub trait DfsVisitor<T: Tree> {
    fn enter(&mut self, node: &T::Node);
    fn exit(&mut self, node: &T::Node);
}

fn default_children<'a, T: Tree>(node: &'a T::Node, tree: &'a T) -> Vec<&'a T::Node> {
    tree.children(node).collect()
}

type ChildFn<T> = for<'b> fn(&'b <T as Tree>::Node, &'b T) -> Vec<&'b <T as Tree>::Node>;
pub struct PostOrder<'a, T: Tree> {
    dfs: Dfs<'a, T, ChildFn<T>>,
}

impl<'a, T: Tree> PostOrder<'a, T> {
    fn new(tree: &'a T) -> Self {
        Self {
            dfs: Dfs::new(tree, default_children::<T>),
        }
    }
}

impl<'a, T: Tree> Iterator for PostOrder<'a, T> {
    type Item = &'a T::Node;

    fn next(&mut self) -> Option<Self::Item> {
        for event in self.dfs.by_ref() {
            if let DfsEvent::Exit(node) = event {
                return Some(node);
            }
        }
        None
    }
}

pub struct PreOrder<'a, T: Tree> {
    dfs: Dfs<'a, T, ChildFn<T>>,
}

impl<'a, T: Tree> PreOrder<'a, T> {
    fn new(tree: &'a T) -> Self {
        Self {
            dfs: Dfs::new(tree, default_children::<T>),
        }
    }
}

impl<'a, T: Tree> Iterator for PreOrder<'a, T> {
    type Item = &'a T::Node;

    fn next(&mut self) -> Option<Self::Item> {
        for event in self.dfs.by_ref() {
            if let DfsEvent::Enter(node) = event {
                return Some(node);
            }
        }
        None
    }
}

pub trait TreeTraversal: Tree {
    fn run_dfs<V>(&self, visitor: &mut V)
    where
        V: DfsVisitor<Self>,
        Self: Sized,
    {
        let dfs = Dfs::new(self, default_children::<Self>);

        for event in dfs {
            match event {
                DfsEvent::Enter(n) => visitor.enter(n),
                DfsEvent::Exit(n) => visitor.exit(n),
            }
        }
    }

    fn run_dfs_ordered<V, F>(&self, visitor: &mut V, child_fn: F)
    where
        V: DfsVisitor<Self>,
        F: for<'a> FnMut(&'a Self::Node, &'a Self) -> Vec<&'a Self::Node>,
        Self: Sized,
    {
        let dfs = Dfs::new(self, child_fn);

        for event in dfs {
            match event {
                DfsEvent::Enter(n) => visitor.enter(n),
                DfsEvent::Exit(n) => visitor.exit(n),
            }
        }
    }

    fn post_order(&self) -> PostOrder<'_, Self>
    where
        Self: Sized,
    {
        PostOrder::new(self)
    }
    fn pre_order(&self) -> PreOrder<'_, Self>
    where
        Self: Sized,
    {
        PreOrder::new(self)
    }
}

impl<T: Tree> TreeTraversal for T {}

pub trait TreeHeights: TreeTraversal {
    fn depths(&self) -> HashMap<Self::Node, usize>
    where
        Self: Sized,
        Self::Node: Eq + Hash + Copy,
    {
        let mut layers: HashMap<Self::Node, usize> = HashMap::new();
        let root = self.root();

        let mut stack = vec![(root, 0)];
        layers.insert(*root, 0);

        while let Some((node, depth)) = stack.pop() {
            for child in self.children(node) {
                let d = depth + 1;
                layers.insert(*child, d);
                stack.push((child, d));
            }
        }

        layers
    }
}

impl<T: TreeTraversal> TreeHeights for T {}

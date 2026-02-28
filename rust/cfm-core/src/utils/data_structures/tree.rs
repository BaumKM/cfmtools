mod index_tree;
mod summary;
mod traversal;

pub use index_tree::*;
pub use summary::*;
pub use traversal::*;

pub trait Tree {
    type Node;
    type Children<'a>: Iterator<Item = &'a Self::Node>
    where
        Self: 'a;

    fn size(&self) -> usize;

    fn root(&self) -> &Self::Node;

    fn children<'a>(&'a self, node: &'a Self::Node) -> Self::Children<'a>;

    fn is_leaf(&self, node: &Self::Node) -> bool;
}

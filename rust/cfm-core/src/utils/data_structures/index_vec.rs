use serde::Serialize;

/// A logical index that maps to and from a linear storage index.
///
/// Implementations define how a logical identifier (for example a typed ID
/// or a multi-dimensional index) is converted into a `usize` for indexing
/// contiguous storage.
pub trait Index: Copy + Eq + std::hash::Hash {
    /// Maps this logical index to a linear `usize` index.
    fn to_usize(self) -> usize;
    /// Constructs a logical index from a linear `usize` index.
    fn from_usize(u: usize) -> Self;
}

/// A vector indexed by a logical index type.
///
/// Elements are stored contiguously, while the index type `K` defines how
/// logical indices map to the underlying linear storage.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IndexVec<K: Index, T> {
    data: Vec<T>,
    _marker: std::marker::PhantomData<K>,
}

impl<K: Index, T> IndexVec<K, T> {
    /// Creates a new `IndexVec` from the given data.
    #[must_use]
    pub fn new(data: Vec<T>) -> Self {
        Self {
            data,
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns the number of elements in the vector.
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the vector contains no elements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Returns a reference to the element with the given id, if it exists.
    pub fn get(&self, id: K) -> Option<&T> {
        self.data.get(id.to_usize())
    }

    /// Returns a mutable reference to the element with the given id, if it exists.
    pub fn get_mut(&mut self, id: K) -> Option<&mut T> {
        self.data.get_mut(id.to_usize())
    }

    /// Appends a value to the end of the vector.
    ///
    /// The newly added element is assigned the next logical index.
    pub fn push(&mut self, value: T) {
        self.data.push(value);
    }

    /// Iterates over all elements, yielding `(index, &value)`.
    pub fn enumerate(&self) -> impl Iterator<Item = (K, &T)> {
        self.data
            .iter()
            .enumerate()
            .map(|(i, v)| (K::from_usize(i), v))
    }

    /// Iterates over all elements mutably, yielding `(index, &mut value)`.
    pub fn enumerate_mut(&mut self) -> impl Iterator<Item = (K, &mut T)> {
        self.data
            .iter_mut()
            .enumerate()
            .map(|(i, v)| (K::from_usize(i), v))
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.data.iter_mut()
    }
}

impl<'a, K: Index, T> IntoIterator for &'a IndexVec<K, T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

impl<'a, K: Index, T> IntoIterator for &'a mut IndexVec<K, T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter_mut()
    }
}

impl<K: Index, T> IntoIterator for IndexVec<K, T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

impl<K: Index, T> std::ops::Index<K> for IndexVec<K, T> {
    type Output = T;

    fn index(&self, id: K) -> &Self::Output {
        &self.data[id.to_usize()]
    }
}

impl<K: Index, T> std::ops::IndexMut<K> for IndexVec<K, T> {
    fn index_mut(&mut self, id: K) -> &mut Self::Output {
        &mut self.data[id.to_usize()]
    }
}

impl<K: Index, T> std::ops::Index<&K> for IndexVec<K, T> {
    type Output = T;

    fn index(&self, id: &K) -> &Self::Output {
        &self.data[id.to_usize()]
    }
}

impl<K: Index, T> std::ops::IndexMut<&K> for IndexVec<K, T> {
    fn index_mut(&mut self, id: &K) -> &mut Self::Output {
        &mut self.data[id.to_usize()]
    }
}

impl<K: Index, T> From<Vec<T>> for IndexVec<K, T> {
    fn from(data: Vec<T>) -> Self {
        Self::new(data)
    }
}

impl<K: Index, T> IndexVec<K, T> {
    /// Maps each element using `f`, preserving the index type.
    pub fn map<U, F>(self, f: F) -> IndexVec<K, U>
    where
        F: FnMut(T) -> U,
    {
        IndexVec::new(self.data.into_iter().map(f).collect())
    }
}

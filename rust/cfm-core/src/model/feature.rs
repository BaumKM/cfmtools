use std::{borrow::Borrow, fmt};

use crate::utils::data_structures::{Index, IndexVec};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Feature {
    id: usize,
}

impl Feature {
    #[must_use]
    pub fn new(id: usize) -> Self {
        Self::from_usize(id)
    }
}

impl Index for Feature {
    fn to_usize(self) -> usize {
        self.id
    }

    fn from_usize(u: usize) -> Self {
        Self { id: u }
    }
}
impl fmt::Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FeatureName {
    name: String,
}

impl FeatureName {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl From<&str> for FeatureName {
    fn from(s: &str) -> Self {
        Self { name: s.to_owned() }
    }
}

impl Borrow<str> for FeatureName {
    fn borrow(&self) -> &str {
        &self.name
    }
}

pub type FeatureVec<T> = IndexVec<Feature, T>;

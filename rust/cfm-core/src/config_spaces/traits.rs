use std::fmt::Debug;

use rug::Integer;
use serde_json::Value;

use crate::model::{cfm::CFM, feature::FeatureVec};

/// Represents a general type of configuration for a `CFM`.
pub trait Configuration: Clone + Debug {
    /// Global feature counts for this configuration
    fn feature_counts(&self, model: &CFM) -> &FeatureVec<usize>;

    /// Pretty print the configuration using the given model.
    fn pretty_print(&self, model: &CFM) -> String;

    /// Serialize the configuration using the given model.
    fn serialize(&self, model: &CFM) -> Value;
}

/// Represents the configuration space of a `CFM`.
///
/// The configuration space consists of all valid configurations of the model.
/// Cross-tree constraints are ignored.
pub trait ConfigSpace {
    /// Concrete type of the configurations.
    type Config: Configuration;

    /// Preprocessed data needed for queries.
    type Cache: Send + Sync;

    /// Build all preprocessing structures required by this configuration space.
    fn build_cache(&self) -> Self::Cache;

    /// Total number of valid configurations (ignores cross-tree constraints).
    fn count(&self, cache: &Self::Cache) -> Integer;

    /// Compute the rank of a valid configuration (ignores cross-tree constraints).
    ///
    /// The returned rank must lie in the interval `[0, count(setup))`.
    fn rank(&self, cache: &Self::Cache, config: &Self::Config) -> Integer;

    /// Reconstruct the configuration with the given rank (ignores cross-tree constraints).
    ///
    /// The rank must satisfy `rank < count(setup)`.
    fn unrank(&self, cache: &Self::Cache, rank: &Integer) -> Self::Config;
}

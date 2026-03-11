use rand::Rng;
use rug::Integer;
use serde::Serialize;

use crate::{
    config_spaces::ConfigSpace,
    model::feature::FeatureVec,
    utils::{data_structures::TreeSummary, sampling::CompoundRng},
};

pub mod general;
pub mod structural;
use std::time::Duration;

pub trait UniformSampler {
    type Space: ConfigSpace;

    /// Sampler-specific preprocessing cache.
    type SamplerCache;

    /// Statistics about the sampling process
    type Statistics: SampleStatistics;

    /// Build the sampler cache from the base space cache.
    fn build_sampler_cache(
        config_space: &Self::Space,
        config_cache: &<Self::Space as ConfigSpace>::Cache,
    ) -> Self::SamplerCache;

    fn sample<R: Rng>(
        &self,
        config_cache: &<Self::Space as ConfigSpace>::Cache,
        sampler_cache: &Self::SamplerCache,
        rng: &mut CompoundRng<R>,
    ) -> SampleResult<<Self::Space as ConfigSpace>::Config, Self::Statistics>;

    fn configuration_space(&self) -> &Self::Space;
}

#[derive(Debug, Clone)]
pub struct SampleResult<C, S> {
    pub value: C,
    pub statistics: S,
}

pub trait SampleStatistics: Clone + Serialize {
    fn accumulate(&mut self, other: Self);
}

#[derive(Debug, Clone)]
pub struct UnconstrainedSummary {
    /// For each feature: number of unconstrained configurations below it
    pub config_counts: FeatureVec<Integer>,

    /// Average configuration size below each feature
    pub avg_config_sizes: FeatureVec<f64>,

    /// Time spent building count DP tables
    pub count_dp_build_time: Duration,

    /// Time spent building size DP tables
    pub size_dp_build_time: Duration,

    /// Structural tree summary
    pub tree_summary: TreeSummary,

    /// Number of cross-tree constraints
    pub number_of_cross_tree_constraints: usize,
}

#[derive(Debug, Clone)]
pub struct ConstrainedEnumerationSummary {
    /// Time spent building count DP + ranking cache
    pub count_dp_build_time: Duration,

    /// Time limit for enumeration.
    pub time_limit: Duration,

    /// Number of unconstrained configurations enumerated.
    pub enumerated: usize,

    /// Number of enumerated configurations valid under cross-tree constraints
    pub valid: usize,

    /// Ratio of valid to enumerated configurations.
    pub valid_ratio: f64,

    /// Average size of valid configurations (if any).
    pub avg_valid_size: Option<f64>,

    /// Time until the first valid configuration is found.
    pub time_to_first_valid: Option<Duration>,

    /// Whether enumeration finished or was cut short
    pub status: EnumerationStatus,

    /// For each enumerated unconstrained rank: whether it satisfies cross tree constraints
    pub rank_cross_tree_validity: Option<Vec<bool>>,
}

#[derive(Debug, Clone)]
pub enum EnumerationStatus {
    Finished {
        enumeration_time: Duration,
    },
    Incomplete {
        enumeration_time: Duration,
        estimated_enumeration_time: MaybeDuration,
    },
}

/// A duration that may overflow the representable range of `Duration`.
#[derive(Debug, Clone, Copy)]
pub enum MaybeDuration {
    Finite(Duration),
    Infinite,
}

impl MaybeDuration {
    #[must_use]
    pub fn from_seconds(seconds: f64) -> Self {
        if seconds > Duration::MAX.as_secs_f64() {
            Self::Infinite
        } else {
            Self::Finite(Duration::from_secs_f64(seconds))
        }
    }
}

impl From<Duration> for MaybeDuration {
    fn from(value: Duration) -> Self {
        Self::Finite(value)
    }
}

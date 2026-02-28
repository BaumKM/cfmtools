use std::time::Duration;

use rug::Integer;
use serde::Serialize;

use crate::{algorithms::UniformSampler, model::cfm::CFM};

pub mod structural;

#[derive(Clone, Debug)]
pub struct BenchmarkParams {
    /// Number of samples per run
    pub samples: usize,

    /// Number of independent runs
    pub runs: usize,

    /// Base RNG seed
    pub seed: u64,

    /// Enable calculating the constrained configuration space size.
    /// Only use when domain size is reasonably small.
    pub calculate_constrained_space_size: bool,
}

pub trait Benchmark: Send + Sync {
    type Sampler: UniformSampler;
    fn run(
        &self,
        cfm: &CFM,
        params: &BenchmarkParams,
    ) -> BenchmarkResult<<Self::Sampler as UniformSampler>::Statistics>;
}

#[derive(Clone, Debug)]
pub struct BenchmarkResult<S> {
    pub runs: Vec<RunResult<S>>,
}

#[derive(Clone, Debug)]
pub struct RunResult<S> {
    pub runtime: RuntimeResult,
    pub uniformity: UniformityResult,
    /// Aggregated sampler-specific statistics for this run
    pub sampler_stats: S,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeResult {
    /// Time spent building caches / setup
    pub setup_time: Duration,

    /// Time spent in the sampling loop (sampler.sample only)
    pub sampling_time: Duration,

    /// Time spent computing rankings (`config_space.rank`)
    pub ranking_time: Duration,
}

#[derive(Clone, Debug)]
pub enum UniformityResult {
    /// The constrained configuration space size is known exactly
    KnownSupport {
        constrained_space_size: Integer,
        samples: usize,

        /// Sparse observed distribution (rank, count)
        distribution: Vec<(Integer, usize)>,

        chi_square: f64,
        chi_square_pvalue: f64,
        total_variation: f64,
        max_deviation: f64,
    },
    UnknownSupport {
        /// Total number of samples observed
        samples: usize,

        /// Maximum observed probability mass: `max_i` (`c_i` / n)
        ///
        /// Detects dominant outcomes.
        p_max: f64,

        /// Collision probability: `sum_i` (`c_i` / n)^2
        ///
        /// Probability that two independent samples are equal.
        collision_probability: f64,

        /// Effective number of bins: 1 / `collision_probability`
        ///
        /// Number of equally likely bins that would produce the same collision rate.
        effective_bins: f64,

        /// Sparse observed distribution: (value, count)
        distribution: Vec<(Integer, usize)>,
    },
}

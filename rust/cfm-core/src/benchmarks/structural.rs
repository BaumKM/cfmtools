use rug::{Complete, Integer};
use statrs::distribution::{ChiSquared, ContinuousCDF};

use crate::{
    algorithms::{
        SampleResult, SampleStatistics, UniformSampler,
        general::sampling::{RankingStatistics, UniformRankingSampler},
        structural::sampling::{BacktrackingStatistics, UniformBacktrackingSampler},
    },
    benchmarks::{
        Benchmark, BenchmarkParams, BenchmarkResult, RunResult, RuntimeResult, UniformityResult,
    },
    config_spaces::{ConfigSpace, Configuration, structural::StructuralConfigSpace},
    model::cfm::CFM,
};

use rand::{Rng, SeedableRng, rngs::StdRng};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

pub struct RankingBenchmark {
    pub sampler: UniformRankingSampler<StructuralConfigSpace>,
}

impl Benchmark for RankingBenchmark {
    type Sampler = UniformRankingSampler<StructuralConfigSpace>;

    fn run(&self, cfm: &CFM, params: &BenchmarkParams) -> BenchmarkResult<RankingStatistics> {
        run_sampler_benchmark(&self.sampler, cfm, params)
    }
}

pub struct BacktrackingBenchmark {
    pub sampler: UniformBacktrackingSampler,
}

impl Benchmark for BacktrackingBenchmark {
    type Sampler = UniformBacktrackingSampler;

    fn run(&self, cfm: &CFM, params: &BenchmarkParams) -> BenchmarkResult<BacktrackingStatistics> {
        run_sampler_benchmark(&self.sampler, cfm, params)
    }
}

fn run_sampler_benchmark<U>(
    sampler: &U,
    cfm: &CFM,
    params: &BenchmarkParams,
) -> BenchmarkResult<U::Statistics>
where
    U: UniformSampler,
    U::Statistics: SampleStatistics,
    U::Space: ConfigSpace,
{
    let config_space = sampler.configuration_space();
    let config_cache = <U::Space as ConfigSpace>::build_cache(config_space);

    let unconstrained_space_size: Integer = config_space.count(&config_cache);
    assert!(!unconstrained_space_size.is_zero());

    let mut seed_rng = StdRng::seed_from_u64(params.seed);
    let mut runs = Vec::with_capacity(params.runs);

    for _run_idx in 0..params.runs {
        let seed: u64 = seed_rng.next_u64();
        let mut rng = StdRng::seed_from_u64(seed).into();

        // --------------------
        // Setup phase
        // --------------------
        let setup_start = Instant::now();
        let sampler_cache = U::build_sampler_cache(config_space, &config_cache);
        let setup_time = setup_start.elapsed();

        // --------------------
        // Sampling phase
        // --------------------
        let mut counts: HashMap<Integer, usize> = HashMap::new();
        let mut sampler_stats: Option<U::Statistics> = None;

        let mut sampling_time: Duration = Duration::ZERO;
        let mut ranking_time: Duration = Duration::ZERO;

        for _ in 0..params.samples {
            let sampling_start = Instant::now();
            let SampleResult { value, statistics } =
                sampler.sample(&config_cache, &sampler_cache, &mut rng);
            sampling_time += sampling_start.elapsed();

            let ranking_start = Instant::now();
            let rank = config_space.rank(&config_cache, &value);
            ranking_time += ranking_start.elapsed();
            *counts.entry(rank).or_insert(0) += 1;

            match &mut sampler_stats {
                Some(acc) => acc.accumulate(statistics),
                None => sampler_stats = Some(statistics),
            }
        }

        let constrained_space_size: Option<Integer> = if !cfm.has_cross_tree_constraints() {
            // No constraints → same as unconstrained
            Some(unconstrained_space_size.clone())
        } else if params.calculate_constrained_space_size {
            // Constraints exist and exact computation requested
            Some(compute_constrained_space_size(
                config_space,
                &config_cache,
                cfm,
            ))
        } else {
            // Constraints exist but exact size not requested
            None
        };

        let uniformity = compute_uniformity(&counts, params.samples, constrained_space_size);

        runs.push(RunResult {
            runtime: RuntimeResult {
                setup_time,
                sampling_time,
                ranking_time,
            },
            uniformity,
            sampler_stats: sampler_stats.expect("params.samples must be > 0"),
        });
    }

    BenchmarkResult { runs }
}

fn compute_constrained_space_size<S: ConfigSpace>(
    config_space: &S,
    cache: &S::Cache,
    cfm: &CFM,
) -> Integer {
    let total_unconstrained = config_space.count(cache);
    let mut constrained = Integer::from(0);
    let one = Integer::from(1);

    let mut rank = Integer::from(0);
    while rank < total_unconstrained {
        let cfg = config_space.unrank(cache, &rank);
        let counts = cfg.feature_counts(cfm);

        if cfm.satisfies_cross_tree_constraints(counts) {
            constrained += &one;
        }

        rank += &one;
    }

    constrained
}

fn compute_uniformity(
    counts: &HashMap<Integer, usize>,
    samples: usize,
    constrained_space_size: Option<Integer>,
) -> UniformityResult {
    let mut distribution: Vec<(Integer, usize)> =
        counts.iter().map(|(k, &v)| (k.clone(), v)).collect();

    distribution.sort_by(|a, b| a.0.cmp(&b.0));

    let n = samples as f64;

    if let Some(constrained_space_size) = constrained_space_size {
        let m_f64 = constrained_space_size.to_f64();

        let expected = n / m_f64;
        let inv_n = 1.0 / n;

        let mut chi_square = 0.0f64;
        let mut max_dev = 0.0f64;
        let mut tv_sum = 0.0f64;

        // Non-zero bins
        for &c in counts.values() {
            let c_f = c as f64;
            let diff = c_f - expected;
            let abs_diff = diff.abs();

            chi_square += diff * diff / expected;
            max_dev = max_dev.max(abs_diff);
            tv_sum += abs_diff * inv_n;
        }

        // Zero bins
        let observed_bins = Integer::from(counts.len());
        let zero_bins = if constrained_space_size > observed_bins {
            (&constrained_space_size - &observed_bins).complete()
        } else {
            Integer::from(0)
        };

        if !zero_bins.is_zero() {
            let zero_bins_f64 = zero_bins.to_f64();

            let abs_diff = expected;

            chi_square += zero_bins_f64 * (expected * expected / expected);
            max_dev = max_dev.max(abs_diff);
            tv_sum += zero_bins_f64 * abs_diff * inv_n;
        }

        let total_variation = 0.5 * tv_sum;

        // Degrees of freedom = m - 1
        let chi_square_pvalue = if constrained_space_size <= 1 {
            1.0
        } else {
            let dof_big = &constrained_space_size - Integer::from(1);
            let dof = dof_big.to_f64();
            let chi_dist = ChiSquared::new(dof).unwrap();
            1.0 - chi_dist.cdf(chi_square)
        };

        UniformityResult::KnownSupport {
            constrained_space_size,
            samples,
            distribution,
            chi_square,
            chi_square_pvalue,
            total_variation,
            max_deviation: max_dev,
        }
    } else {
        let mut c_max: usize = 0;
        let mut sum_c2: f64 = 0.0;

        for &(_, c) in &distribution {
            let cf = c as f64;

            c_max = c_max.max(c);
            sum_c2 += cf * cf;
        }

        let collision_probability = if n > 0.0 {
            sum_c2 / (n * n) // Σ p_i²
        } else {
            0.0
        };

        let effective_bins = if collision_probability > 0.0 {
            1.0 / collision_probability
        } else {
            0.0
        };

        let p_max = if n > 0.0 { c_max as f64 / n } else { 0.0 };

        UniformityResult::UnknownSupport {
            samples,
            p_max,
            collision_probability,
            effective_bins,
            distribution,
        }
    }
}

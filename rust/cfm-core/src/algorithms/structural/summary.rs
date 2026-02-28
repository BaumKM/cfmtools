use std::time::Duration;
use std::time::Instant;

use rug::Integer;

use crate::algorithms::ConstrainedEnumerationSummary;
use crate::algorithms::EnumerationStatus;

use crate::algorithms::MaybeDuration;

use crate::algorithms::UnconstrainedSummary;
use crate::utils::data_structures::TreeStatistics;
use crate::{
    config_spaces::{ConfigSpace, structural::StructuralConfigSpace},
    model::feature::FeatureVec,
};

impl StructuralConfigSpace {
    #[must_use]
    pub fn summarize_unconstrained(&self) -> UnconstrainedSummary {
        let cfm = self.cfm();

        /* ---------------- Count DP ---------------- */

        let count_start = Instant::now();
        let cache = self.build_dp_cache();
        let count_dp_build_time = count_start.elapsed();

        let config_counts = cache.total_config_counts().clone();

        /* ---------------- Size DP ---------------- */

        let size_start = Instant::now();
        let avg_config_sizes: FeatureVec<f64> = self
            .compute_expected_config_sizes(&cache)
            .map(|r| r.to_f64());
        let size_dp_build_time = size_start.elapsed();

        UnconstrainedSummary {
            config_counts,
            avg_config_sizes,
            count_dp_build_time,
            size_dp_build_time,
            tree_summary: cfm.tree_summary(),
            number_of_cross_tree_constraints: cfm.number_of_cross_tree_constraints(),
        }
    }

    #[must_use]
    pub fn enumerate_constrained(
        &self,
        time_limit: Duration,
        show_rank_validity: bool,
    ) -> ConstrainedEnumerationSummary {
        let cfm = self.cfm();

        /* ---------------- Count DP build ---------------- */

        let dp_start = Instant::now();
        let cache = self.build_cache();
        let count_dp_build_time = dp_start.elapsed();

        let total_unconstrained = self.count(&cache);

        /* ---------------- Enumeration ---------------- */

        let enumeration_start = Instant::now();
        let max_iters: usize = total_unconstrained.to_usize().unwrap_or(usize::MAX);

        let mut enumerated = 0usize;
        let mut valid = 0usize;
        let mut sum_size_valid = 0usize;
        let mut time_to_first_valid = None;

        let mut rank_cross_tree_validity = if show_rank_validity {
            Some(Vec::with_capacity(max_iters))
        } else {
            None
        };

        // ---- initialize cursor once ----
        let mut cursor = self.unrank_into_cursor(&cache, &Integer::from(0));

        loop {
            if enumeration_start.elapsed() >= time_limit {
                break;
            }

            if enumerated >= max_iters {
                break;
            }
            let feature_counts = cursor.feature_counts();
            let size: usize = feature_counts.iter().sum();

            let is_valid = cfm.satisfies_cross_tree_constraints(feature_counts);

            if let Some(ref mut validity) = rank_cross_tree_validity {
                validity.push(is_valid);
            }

            if is_valid {
                valid += 1;
                sum_size_valid += size;

                if time_to_first_valid.is_none() {
                    time_to_first_valid = Some(enumeration_start.elapsed());
                }
            }

            enumerated += 1;

            let ok = self.increment_cursor(&cache, &mut cursor);
            if !ok {
                break; // exhausted
            }
        }

        let enumeration_time = enumeration_start.elapsed();
        let finished = enumerated == total_unconstrained;

        /* ---------------- Estimation ---------------- */

        let status = if finished {
            EnumerationStatus::Finished { enumeration_time }
        } else {
            let elapsed_secs = enumeration_time.as_secs_f64();
            let rate = if elapsed_secs > 0.0 {
                enumerated as f64 / elapsed_secs
            } else {
                0.0
            };

            let estimated_secs = if rate > 0.0 {
                total_unconstrained.to_f64() / rate
            } else {
                f64::INFINITY
            };

            EnumerationStatus::Incomplete {
                enumeration_time,
                estimated_enumeration_time: MaybeDuration::from_seconds(estimated_secs),
            }
        };

        let valid_ratio = if enumerated == 0 {
            0.0
        } else {
            valid as f64 / enumerated as f64
        };

        let avg_valid_size = if valid == 0 {
            None
        } else {
            Some(sum_size_valid as f64 / valid as f64)
        };

        ConstrainedEnumerationSummary {
            count_dp_build_time,
            time_limit,
            enumerated,
            valid,
            valid_ratio,
            avg_valid_size,
            time_to_first_valid,
            status,
            rank_cross_tree_validity,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{model::cfm::CFM, test_cfms::TestCFM, utils::data_structures::Tree};

    use super::*;

    /// Numerically compare floats with tolerance.
    fn assert_close(a: f64, b: f64, eps: f64, msg: &str) {
        let diff = (a - b).abs();
        assert!(
            diff <= eps,
            "{msg} | a = {a}, b = {b}, diff = {diff}, eps = {eps}"
        );
    }

    /// Ground-truth computation by enumerating every configuration.
    /// Returns the average size of a configuration of the cfm.
    fn enumerate_average_config_size(config_space: &StructuralConfigSpace) -> f64 {
        let cache = <StructuralConfigSpace as ConfigSpace>::build_cache(config_space);
        let total: usize = config_space.count(&cache).try_into().unwrap();

        let mut sum_sizes = Integer::from(0);

        for rank in 0..total {
            let cfg = config_space.unrank(&cache, &Integer::from(rank));
            let size: Integer = cfg.size().into();
            sum_sizes += size;
        }

        sum_sizes.to_f64() / total as f64
    }

    fn assert_config_statistics_correct(cfm: Arc<CFM>) {
        let config_space = StructuralConfigSpace::new(cfm.clone());

        // Build DP cache once
        let cache = config_space.build_dp_cache();

        // Enumerate (ground truth via enumeration logic)
        let expected = enumerate_average_config_size(&config_space);

        let actual = config_space
            .compute_expected_config_sizes(&cache)
            .map(|r| r.to_f64())[cfm.root()];

        let eps = 1e-9;
        assert_close(
            actual,
            expected,
            eps,
            "Avg root configuration size mismatch (DP vs enumeration)",
        );
    }

    #[test]
    fn config_stats_simple_cfm() {
        assert_config_statistics_correct(TestCFM::build_simple_cfm());
    }

    #[test]
    fn config_stats_wide_cfm() {
        assert_config_statistics_correct(TestCFM::build_wide_cfm());
    }

    #[test]
    fn config_stats_deep_cfm() {
        assert_config_statistics_correct(TestCFM::build_deep_cfm());
    }

    #[test]
    fn config_stats_gap_cfm() {
        assert_config_statistics_correct(TestCFM::build_gap_cfm());
    }

    #[test]
    fn config_stats_large_gap_cfm() {
        assert_config_statistics_correct(TestCFM::build_large_gap_cfm());
    }

    #[test]
    fn config_stats_cutoff_cfm() {
        assert_config_statistics_correct(TestCFM::build_cutoff_cfm());
    }

    #[test]
    fn config_stats_deep_chain_cfm() {
        assert_config_statistics_correct(TestCFM::build_deep_chain_cfm());
    }

    #[test]
    fn config_stats_group_restricted_cfm() {
        assert_config_statistics_correct(TestCFM::build_group_restricted_cfm());
    }

    #[test]
    fn config_stats_dead_branch_cfm() {
        assert_config_statistics_correct(TestCFM::build_dead_branch_cfm());
    }

    #[test]
    fn config_stats_double_invalid_cutoff_cfm() {
        assert_config_statistics_correct(TestCFM::build_double_invalid_cutoff_cfm());
    }
}

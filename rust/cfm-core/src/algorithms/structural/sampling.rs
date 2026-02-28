use std::sync::Arc;

use rand::{Rng, RngExt};
use rug::{Complete, Integer};
use serde::Serialize;

use crate::{
    algorithms::{SampleResult, SampleStatistics, UniformSampler},
    config_spaces::{
        ConfigSpace, Configuration as _,
        structural::{StructuralBuilder, StructuralConfigSpace, StructuralNode, max_card},
    },
    model::feature::{Feature, FeatureVec},
    utils::{
        data_structures::{Tree, TreeTraversal},
        sampling::{AliasTable, CompoundRng},
    },
};

/// Sampler-specific cache for the backtracking sampler.
///
/// Stores alias tables derived from cumulative DP structures.
pub struct BacktrackingSamplerCache {
    /// For each feature f:
    /// Alias table over realized group cardinalities (c,k).
    group_tables: FeatureVec<Option<AliasTable<(usize, usize)>>>,

    /// For each feature f:
    /// Alias tables for multiplicity choice `r_i`.
    multiplicity_tables: FeatureVec<MultiplicityTables>,
}

#[derive(Debug, Clone)]
struct MultiplicityTables {
    n_children: usize,
    max_c: usize,
    max_k: usize,
    /// length = `n_children` * (`max_c+1`) * (`max_k+1`)
    tables: Vec<Option<AliasTable<usize>>>,
}

impl MultiplicityTables {
    #[inline]
    fn blocks_per_model(&self) -> usize {
        (self.max_c + 1) * (self.max_k + 1)
    }

    /// Converts 1-based model index to 0-based and returns
    /// the starting flat index of that model.
    #[inline]
    fn model_base(&self, i: usize) -> usize {
        debug_assert!(i >= 1 && i <= self.n_children);
        (i - 1) * self.blocks_per_model()
    }

    /// Returns the flat index for block `(c, k)` in model `i`.
    #[inline]
    fn index(&self, i: usize, c: usize, k: usize) -> usize {
        debug_assert!(c <= self.max_c);
        debug_assert!(k <= self.max_k);
        self.model_base(i) + c * (self.max_k + 1) + k
    }

    #[inline]
    fn table(&self, i: usize, c_rem: usize, k_rem: usize) -> Option<&AliasTable<usize>> {
        let idx = self.index(i, c_rem, k_rem);
        self.tables[idx].as_ref()
    }
}

/// Uniform sampler that implements UniSample-BT (no cross-tree constraints).
pub struct UniformBacktrackingSampler {
    config_space: StructuralConfigSpace,
}

impl UniformBacktrackingSampler {
    #[must_use]
    pub fn new(config_space: StructuralConfigSpace) -> Self {
        Self { config_space }
    }
}

impl UniformSampler for UniformBacktrackingSampler {
    type Space = StructuralConfigSpace;
    type SamplerCache = BacktrackingSamplerCache;
    type Statistics = BacktrackingStatistics;

    fn build_sampler_cache(
        config_space: &Self::Space,
        config_cache: &<Self::Space as ConfigSpace>::Cache,
    ) -> Self::SamplerCache {
        let cfm = config_space.cfm();
        let n_features = cfm.size();

        let mut group_tables: FeatureVec<Option<AliasTable<(usize, usize)>>> =
            vec![None; n_features].into();
        let mut multiplicity_tables: FeatureVec<MultiplicityTables> = vec![
            MultiplicityTables {
                n_children: 0,
                max_c: 0,
                max_k: 0,
                tables: Vec::new()
            };
            n_features
        ]
        .into();

        for feature in cfm.pre_order() {
            let dp = &config_cache.count_dp_tables()[feature];
            let n_children = cfm.children(feature).len();

            // group tables
            {
                let mut items: Vec<(usize, usize)> = Vec::new();
                let mut weights: Vec<Integer> = Vec::new();

                for c in cfm.group_type_cardinality(feature) {
                    for k in cfm.group_instance_cardinality(feature) {
                        let block_size = dp.get(n_children, c, k);

                        // only need non zero blocks
                        if !block_size.is_zero() {
                            items.push((c, k));
                            weights.push(block_size.clone());
                        }
                    }
                }
                if !items.is_empty() {
                    group_tables[feature] = Some(AliasTable::new(items, &weights));
                }
            }

            // multiplicity tables
            let max_c = max_card(cfm.group_type_cardinality(feature));
            let max_k = max_card(cfm.group_instance_cardinality(feature));

            let blocks_per_model = (max_c + 1) * (max_k + 1);
            let total_slots = n_children * blocks_per_model;

            let cum_grid_sizes = &config_cache.cum_grid_sizes()[feature];

            let mut tables: Vec<Option<AliasTable<usize>>> = Vec::with_capacity(total_slots);
            for grids in cum_grid_sizes.iter_blocks_with_grids() {
                let mut items: Vec<usize> = Vec::new();
                let mut weights: Vec<Integer> = Vec::new();

                let mut prev_cum: &Integer = &Integer::from(0);
                for grid_entry in grids {
                    let grid_size = (grid_entry.cumulative_size() - prev_cum).complete();
                    prev_cum = grid_entry.cumulative_size();

                    // only need non zero multiplicities
                    if !grid_size.is_zero() {
                        items.push(grid_entry.multiplicity());
                        weights.push(grid_size);
                    }
                }

                if items.is_empty() {
                    tables.push(None);
                } else {
                    // iteration order of the cum_grid_size iterator matcher our internal layout
                    tables.push(Some(AliasTable::new(items, &weights)));
                }
            }

            multiplicity_tables[feature] = MultiplicityTables {
                n_children,
                max_c,
                max_k,
                tables,
            };
        }

        BacktrackingSamplerCache {
            group_tables,
            multiplicity_tables,
        }
    }

    fn sample<R: Rng>(
        &self,
        _config_cache: &<Self::Space as ConfigSpace>::Cache,
        sampler_cache: &Self::SamplerCache,
        rng: &mut CompoundRng<R>,
    ) -> SampleResult<<Self::Space as ConfigSpace>::Config, BacktrackingStatistics> {
        let cfm = self.config_space.cfm();
        let root = self.config_space.cfm().root();
        let mut sample_rejections: usize = 0;
        let mut multiset_rejections: FeatureVec<usize> = vec![0usize; cfm.size()].into();

        loop {
            let mut builder = StructuralBuilder::new(cfm.size());

            let root_group = self.sample_recursively(
                root,
                sampler_cache,
                &mut LogFactorials::new(0),
                &mut builder,
                &mut multiset_rejections,
                rng,
            );

            let cfg = builder.finish(root_group);

            //check cross tree constraints
            let counts = cfg.feature_counts(cfm);
            if !cfm.satisfies_cross_tree_constraints(counts) {
                sample_rejections += 1;
                continue;
            }

            return SampleResult {
                value: cfg,
                statistics: BacktrackingStatistics {
                    sample_rejections,
                    multiset_rejections,
                },
            };
        }
    }

    fn configuration_space(&self) -> &Self::Space {
        &self.config_space
    }
}

impl UniformBacktrackingSampler {
    fn sample_recursively<R: Rng>(
        &self,
        f: &Feature,
        sampler_cache: &BacktrackingSamplerCache,
        log_factorials: &mut LogFactorials,
        builder: &mut StructuralBuilder,
        multiset_rejections: &mut FeatureVec<usize>,
        rng: &mut CompoundRng<R>,
    ) -> Arc<StructuralNode> {
        if self.config_space.cfm().is_leaf(f) {
            let node = builder.begin_node(f);
            return builder.finish_node(node);
        }

        let children = self.config_space.cfm().children(f);
        let n_children = children.len();

        // Step 1: sample (c,k)
        let &(mut c_rem, mut k_rem) = sampler_cache.group_tables[f]
            .as_ref()
            .expect("feature must have non empty config space")
            .sample(rng);

        // Step 2: sample r_i backwards using multiplicity alias tables
        let mut r: Vec<usize> = vec![0; n_children];

        let mult_tables = &sampler_cache.multiplicity_tables[f];

        // iterate i0 = n_children-1 .. 0, with i = i0+1 (1-based)
        for i0 in (0..n_children).rev() {
            let i = i0 + 1;

            let chosen = *mult_tables
                .table(i, c_rem, k_rem)
                .as_ref()
                .expect("reachable c_rem, k_rem must lead to at least one config")
                .sample(rng);
            r[i0] = chosen;

            if chosen > 0 {
                c_rem -= 1;
            }
            k_rem -= chosen;
        }

        debug_assert!(c_rem == 0 && k_rem == 0);

        // Step 3: generate each child multiset via list-uniform + rejection
        let mut node = builder.begin_node(f);

        for (i, child) in children.enumerate() {
            let ri = r[i];
            if ri == 0 {
                continue;
            }

            let mut number_of_rejections = 0usize;
            let accepted_groups: Vec<Arc<StructuralNode>> = loop {
                let cp = builder.checkpoint();

                let mut list: Vec<Arc<StructuralNode>> = Vec::with_capacity(ri);
                for _ in 0..ri {
                    let g = self.sample_recursively(
                        child,
                        sampler_cache,
                        log_factorials,
                        builder,
                        multiset_rejections,
                        rng,
                    );
                    list.push(g);
                }

                let multiplicities = builder.count_configurations(&list);

                if Self::accept_multiset(&multiplicities, ri, log_factorials, rng.normal_rng_mut())
                {
                    break list;
                }
                // multiset rejected
                number_of_rejections += 1;
                builder.rollback(&cp);
            };

            node = node.add_children(accepted_groups);
            multiset_rejections[child] += number_of_rejections;
        }

        builder.finish_node(node)
    }

    fn accept_multiset<R: Rng>(
        multiplicities: &[usize],
        r: usize,
        log_factorials: &mut LogFactorials,
        rng: &mut R,
    ) -> bool {
        log_factorials.ensure(r);
        // log p = sum_j ln(m_j!) - ln(r!)
        let mut log_p = -log_factorials.log_fact(r);

        for &mj in multiplicities {
            log_p += log_factorials.log_fact(mj);
        }

        // Sample u in [0.0, 1.0)
        let u: f64 = rng.random();

        // Accept iff log(u) <= log_p
        u.ln() <= log_p
    }
}

struct LogFactorials {
    table: Vec<f64>, // table[n] = ln(n!)
}

impl LogFactorials {
    fn new(max_n: usize) -> Self {
        let mut table = Vec::with_capacity(max_n + 1);
        table.push(0.0); // 0! = 1

        for n in 1..=max_n {
            let prev = table[n - 1];
            table.push(prev + (n as f64).ln());
        }

        Self { table }
    }

    /// Ensures that `ln(n!)` is available in the table.
    ///
    /// The table stores a prefix of logarithmic factorials and is extended
    /// incrementally if needed. Since `ln(n!) = ln((n-1)!) + ln(n)`, existing
    /// values are reused and no recomputation is required.
    fn ensure(&mut self, n: usize) {
        let current = self.table.len() - 1;
        if n <= current {
            return;
        }
        for k in (current + 1)..=n {
            let prev = self.table[k - 1];
            self.table.push(prev + (k as f64).ln());
        }
    }

    #[inline]
    fn log_fact(&self, n: usize) -> f64 {
        self.table[n]
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BacktrackingStatistics {
    pub sample_rejections: usize,
    pub multiset_rejections: FeatureVec<usize>,
}

impl Default for BacktrackingStatistics {
    fn default() -> Self {
        Self {
            sample_rejections: Default::default(),
            multiset_rejections: vec![].into(),
        }
    }
}

impl SampleStatistics for BacktrackingStatistics {
    fn accumulate(&mut self, other: Self) {
        self.sample_rejections += other.sample_rejections;

        for (a, b) in self
            .multiset_rejections
            .iter_mut()
            .zip(other.multiset_rejections.iter())
        {
            *a += b;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rand::{SeedableRng, rngs::StdRng};
    use serde_json::Value;

    use crate::{config_spaces::Configuration, model::cfm::CFM, test_cfms::TestCFM};

    use super::*;

    /// Runs the backtracking sampler until all configurations
    /// have been seen at least once.
    fn assert_bt_sampler_covers_all_configs(cfm: Arc<CFM>) {
        // Build structural config space from the CFM
        let config_space = StructuralConfigSpace::new(cfm);

        let sampler = UniformBacktrackingSampler { config_space };

        // Build config cache
        let config_cache =
            <StructuralConfigSpace as ConfigSpace>::build_cache(&sampler.config_space);

        // Build sampler cache
        let sampler_cache = <UniformBacktrackingSampler as UniformSampler>::build_sampler_cache(
            &sampler.config_space,
            &config_cache,
        );

        let total: usize = sampler
            .config_space
            .count(&config_cache)
            .try_into()
            .unwrap();
        assert!(total > 0, "Config space must not be empty");

        let mut rng = StdRng::seed_from_u64(123456).into();
        let mut seen_ranks = HashSet::new();

        let max_iterations = total * 2000;

        for _ in 0..max_iterations {
            let sample = sampler.sample(&config_cache, &sampler_cache, &mut rng);

            // Rank the sampled configuration
            let rank = sampler.config_space.rank(&config_cache, &sample.value);

            seen_ranks.insert(rank);

            if seen_ranks.len() == total {
                return;
            }
        }

        panic!(
            "Backtracking sampler failed to generate all configurations. Seen {}/{}.",
            seen_ranks.len(),
            total
        );
    }

    /// Checks that the backtracking sampler is approximately uniform:
    /// each configuration should appear with frequency within ±15%.
    fn assert_bt_sampler_is_uniform(cfm: Arc<CFM>) {
        // Build structural config space from the CFM
        let config_space = StructuralConfigSpace::new(cfm);
        let sampler = UniformBacktrackingSampler { config_space };

        // Build config cache
        let config_cache =
            <StructuralConfigSpace as ConfigSpace>::build_cache(&sampler.config_space);

        // Build sampler cache
        let sampler_cache = <UniformBacktrackingSampler as UniformSampler>::build_sampler_cache(
            &sampler.config_space,
            &config_cache,
        );

        let total = sampler
            .config_space
            .count(&config_cache)
            .try_into()
            .unwrap();
        assert!(
            total > 1,
            "Need at least 2 configurations for uniformity test"
        );

        // Number of samples:
        // 1000 per configuration
        let samples_per_config = 1_000;
        let samples = total * samples_per_config;

        let mut rng = StdRng::seed_from_u64(123456).into();

        // Count occurrences by rank
        let mut counts = vec![0usize; total];

        for _ in 0..samples {
            let sample = sampler.sample(&config_cache, &sampler_cache, &mut rng);
            let rank: usize = sampler
                .config_space
                .rank(&config_cache, &sample.value)
                .try_into()
                .unwrap();
            counts[rank] += 1;
        }

        let expected = samples as f64 / total as f64;
        let tolerance = expected * 0.15; // ±15%

        for (rank, &count) in counts.iter().enumerate() {
            let diff = (count as f64 - expected).abs();
            assert!(
                diff <= tolerance,
                "Non-uniform sampling detected for rank {rank}: \
             count = {count}, expected ≈ {expected:.1}, tolerance = ±{tolerance:.1}"
            );
        }
    }

    #[test]
    fn bt_sampler_simple_cfm_covers_all_configs() {
        let cfm = TestCFM::build_simple_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());
        assert_bt_sampler_is_uniform(cfm);
    }

    #[test]
    fn bt_sampler_wide_cfm_covers_all_configs() {
        let cfm = TestCFM::build_wide_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());
        assert_bt_sampler_is_uniform(cfm);
    }

    #[test]
    fn bt_sampler_deep_cfm_covers_all_configs() {
        let cfm = TestCFM::build_deep_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());
        assert_bt_sampler_is_uniform(cfm);
    }

    #[test]
    fn bt_sampler_gap_cfm_covers_all_configs() {
        let cfm = TestCFM::build_gap_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());
        assert_bt_sampler_is_uniform(cfm);
    }

    #[test]
    fn bt_sampler_large_gap_cfm_covers_all_configs() {
        let cfm = TestCFM::build_large_gap_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());
        assert_bt_sampler_is_uniform(cfm);
    }

    #[test]
    fn bt_sampler_cutoff_cfm_covers_all_configs() {
        let cfm = TestCFM::build_cutoff_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());
        assert_bt_sampler_is_uniform(cfm);
    }

    #[test]
    fn bt_sampler_deep_chain_cfm_covers_all_configs() {
        let cfm = TestCFM::build_deep_chain_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());
        assert_bt_sampler_is_uniform(cfm);
    }

    #[test]
    fn bt_sampler_group_restricted_cfm_covers_all_configs() {
        let cfm = TestCFM::build_group_restricted_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());
        assert_bt_sampler_is_uniform(cfm);
    }

    #[test]
    fn bt_sampler_dead_branch_cfm_covers_all_configs() {
        let cfm = TestCFM::build_dead_branch_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());

        // --- Additional semantic check: the dead branch is never sampled ---
        let config_space = StructuralConfigSpace::new(cfm.clone());
        let sampler = UniformBacktrackingSampler { config_space };

        let config_cache =
            <StructuralConfigSpace as ConfigSpace>::build_cache(&sampler.config_space);

        let sampler_cache = <UniformBacktrackingSampler as UniformSampler>::build_sampler_cache(
            &sampler.config_space,
            &config_cache,
        );

        let mut rng = StdRng::seed_from_u64(123456).into();

        // Sample many times and ensure "Dead" never appears in the configuration
        for _ in 0..50_000 {
            let sample = sampler.sample(&config_cache, &sampler_cache, &mut rng);

            // Convert to JSON (or debug string) and check that "Dead" is absent.
            let json: Value = sample.value.serialize(&cfm);
            let s = json.to_string();

            assert!(
                !s.contains("\"Dead\""),
                "Dead feature was sampled unexpectedly: {s}"
            );
        }
    }

    #[test]
    fn bt_sampler_double_invalid_cutoff_cfm_covers_all_configs() {
        let cfm = TestCFM::build_double_invalid_cutoff_cfm();
        assert_bt_sampler_covers_all_configs(cfm.clone());

        // --- Additional semantic check: the invalid branch is never sampled ---
        let config_space = StructuralConfigSpace::new(cfm.clone());
        let sampler = UniformBacktrackingSampler { config_space };

        let config_cache =
            <StructuralConfigSpace as ConfigSpace>::build_cache(&sampler.config_space);

        let sampler_cache = <UniformBacktrackingSampler as UniformSampler>::build_sampler_cache(
            &sampler.config_space,
            &config_cache,
        );

        let mut rng = StdRng::seed_from_u64(123456).into();

        // Only one valid configuration exists: Root_1.
        // Ensure Top, Mid, and Leaf never appear.
        for _ in 0..10_000 {
            let sample = sampler.sample(&config_cache, &sampler_cache, &mut rng);

            let json: Value = sample.value.serialize(&cfm);
            let s = json.to_string();

            assert!(
                !s.contains("\"Top\""),
                "Top feature was sampled unexpectedly: {s}"
            );
            assert!(
                !s.contains("\"Mid\""),
                "Mid feature was sampled unexpectedly: {s}"
            );
            assert!(
                !s.contains("\"Leaf\""),
                "Leaf feature was sampled unexpectedly: {s}"
            );
        }
    }
}

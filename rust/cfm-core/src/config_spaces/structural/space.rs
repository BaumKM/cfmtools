use std::sync::Arc;

use rug::{Integer, Rational};

use crate::{
    combinatorics::multiset::Multiset,
    config_spaces::{
        ConfigSpace,
        structural::{
            StructuralConfiguration,
            space::dp::{CountDpTable, compute_count_dp_tables, compute_expected_config_sizes},
        },
    },
    model::{
        cfm::CFM,
        feature::{Feature, FeatureVec},
        interval::CardinalityInterval,
    },
    utils::data_structures::{Tree, TreeTraversal},
};

mod dp;
mod rank;

pub struct StructuralConfigSpace {
    cfm: Arc<CFM>,
}

impl StructuralConfigSpace {
    #[must_use]
    pub fn new(cfm: Arc<CFM>) -> Self {
        Self { cfm }
    }

    #[must_use]
    pub fn cfm(&self) -> &CFM {
        &self.cfm
    }

    #[must_use]
    pub fn compute_expected_config_sizes(&self, cache: &StructuralDpCache) -> FeatureVec<Rational> {
        compute_expected_config_sizes(
            &self.cfm,
            &cache.count_dp_tables,
            &cache.total_config_counts,
        )
    }

    /// Builds only the DP tables required for counting
    /// and expected configuration size computation.
    #[must_use]
    pub fn build_dp_cache(&self) -> StructuralDpCache {
        let (count_dp_tables, total_config_counts) = compute_count_dp_tables(&self.cfm);

        StructuralDpCache {
            count_dp_tables,
            total_config_counts,
        }
    }
}

pub struct StructuralDpCache {
    /// dp[f] = DP table for feature f
    count_dp_tables: FeatureVec<CountDpTable>,

    /// total number of canonical configurations per feature
    total_config_counts: FeatureVec<Integer>,
}

impl StructuralDpCache {
    #[inline]
    #[must_use]
    pub fn count_dp_tables(&self) -> &FeatureVec<CountDpTable> {
        &self.count_dp_tables
    }

    #[inline]
    #[must_use]
    pub fn total_config_counts(&self) -> &FeatureVec<Integer> {
        &self.total_config_counts
    }
}

pub struct StructuralCache {
    // DP cache
    dp_cache: StructuralDpCache,

    /// For each feature f:
    /// `cum_block_sizes`[f] is a cumulative sum array over the block sizes for f.
    cum_block_sizes: FeatureVec<CumBlockSizes>,

    /// For each feature f:
    /// `cum_grid_sizes`[f] contains the cumulative grid sizes for f.
    cum_grid_sizes: FeatureVec<CumGridSizes>,

    /// For each feature f:
    /// `child_pos`[f] = index of f in its parent's children list.
    /// None for root.
    child_positions: FeatureVec<Option<usize>>,
}

impl StructuralCache {
    #[inline]
    #[must_use]
    pub fn count_dp_tables(&self) -> &FeatureVec<CountDpTable> {
        self.dp_cache.count_dp_tables()
    }

    #[inline]
    #[must_use]
    pub fn total_config_counts(&self) -> &FeatureVec<Integer> {
        self.dp_cache.total_config_counts()
    }

    #[inline]
    #[must_use]
    pub fn cum_grid_sizes(&self) -> &FeatureVec<CumGridSizes> {
        &self.cum_grid_sizes
    }
}

#[derive(Debug, Clone)]
pub struct CumBlockSizes {
    /// Each entry is (c, k, `cumulative_size`)
    blocks: Vec<(usize, usize, Integer)>,
}

impl CumBlockSizes {
    /// Returns the cumulative size of all blocks strictly before `(c, k)`
    /// for the full model (i = n).
    fn prefix_before(&self, c: usize, k: usize) -> Integer {
        match self
            .blocks
            .binary_search_by(|&(bc, bk, _)| (bc, bk).cmp(&(c, k)))
        {
            Ok(i) => {
                if i == 0 {
                    Integer::from(0)
                } else {
                    self.blocks[i - 1].2.clone()
                }
            }
            Err(i) => {
                // (c,k) itself is not present — return prefix before insertion point
                if i == 0 {
                    Integer::from(0)
                } else {
                    self.blocks[i - 1].2.clone()
                }
            }
        }
    }

    /// Finds the `(c, k)` block containing `rank` and returns
    /// `(block_local_rank, c, k)` for the full model (i = n).
    ///
    /// Panics if `rank` is out of bounds.
    fn find_block(&self, rank: &Integer) -> (Integer, usize, usize) {
        let j = self
            .blocks
            .binary_search_by(|(_, _, cum)| {
                if cum <= rank {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            })
            .unwrap_err();

        let start = if j == 0 {
            Integer::from(0)
        } else {
            self.blocks[j - 1].2.clone()
        };

        let block_local_rank = rank - start;
        let (c, k, _) = self.blocks[j];

        (block_local_rank, c, k)
    }

    /// Builds cumulative block sizes for feature `f` from its DP table,
    /// storing only the table for i = n.
    fn build(f: &Feature, cfm: &CFM, dp: &CountDpTable) -> Self {
        let n_children = cfm.children(f).count();
        let group_type_size = cfm
            .group_type_cardinality(f)
            .size()
            .expect("intervals must be finite");
        let group_instance_size = cfm
            .group_instance_cardinality(f)
            .size()
            .expect("intervals must be finite");

        let mut blocks: Vec<(usize, usize, Integer)> =
            Vec::with_capacity(group_type_size * group_instance_size);
        let mut acc: Integer = Integer::from(0);

        for c in cfm.group_type_cardinality(f) {
            for k in cfm.group_instance_cardinality(f) {
                let block_size = dp.get(n_children, c, k);
                if block_size.is_zero() {
                    continue;
                }
                acc += block_size;

                blocks.push((c, k, acc.clone()));
            }
        }

        Self { blocks }
    }
}

#[derive(Debug, Clone)]
pub struct CumGridSizes {
    max_c: usize,
    max_k: usize,

    sizes: Vec<CumGridEntry>,
}

#[derive(Debug, Clone)]
struct CumGridEntry {
    grids: Vec<GridEntry>,
}

#[derive(Debug, Clone)]
pub struct GridEntry {
    cumulative: Integer,
    multiplicity: usize,
    fiber_size: Arc<Integer>,
}

impl GridEntry {
    #[inline]
    #[must_use]
    pub fn cumulative_size(&self) -> &Integer {
        &self.cumulative
    }
    #[inline]
    #[must_use]
    pub fn multiplicity(&self) -> usize {
        self.multiplicity
    }
}

impl CumGridSizes {
    #[inline]
    fn blocks_per_model(&self) -> usize {
        (self.max_c + 1) * (self.max_k + 1)
    }

    /// Converts 1-based model index to 0-based and returns
    /// the starting flat index of that model.
    #[inline]
    fn model_base(&self, i: usize) -> usize {
        debug_assert!(i >= 1);
        let model = i - 1; // child pruned models are one-based
        model * self.blocks_per_model()
    }

    /// Returns the flat index for block `(c, k)` in model `i`.
    #[inline]
    fn index(&self, i: usize, c: usize, k: usize) -> usize {
        debug_assert!(c <= self.max_c);
        debug_assert!(k <= self.max_k);

        self.model_base(i) + c * (self.max_k + 1) + k
    }

    /// Returns the cumulative grid list for block `(c, k)` in
    /// the child pruned model `i` (one based).
    ///
    /// Each entry is `(cumulative_size, multiplicity)`.
    #[inline]
    fn get(&self, i: usize, c: usize, k: usize) -> &[GridEntry] {
        let idx = self.index(i, c, k);
        &self.sizes[idx].grids
    }

    /// Returns the cumulative size of all grids strictly before `grid_index`
    /// in the block `(c, k)` of the child pruned model `i`.
    fn prefix_before(&self, i: usize, c: usize, k: usize, grid_index: usize) -> Integer {
        if grid_index == 0 {
            Integer::from(0)
        } else {
            self.get(i, c, k)[grid_index - 1].cumulative.clone()
        }
    }

    /// Finds the grid containing `block_local_rank` in the block `(c, k)` of the
    /// child pruned model `i` and returns `(grid_local_rank, grid, fiber_size)`.
    ///
    /// Panics if `block_local_rank` is out of bounds.
    fn find_grid(
        &self,
        i: usize,
        c: usize,
        k: usize,
        block_local_rank: &Integer,
    ) -> (Integer, usize, &Arc<Integer>) {
        let grids = self.get(i, c, k);

        let j = grids
            .binary_search_by(|g| {
                if g.cumulative <= *block_local_rank {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            })
            .unwrap_err();

        let start = if j == 0 {
            Integer::from(0)
        } else {
            grids[j - 1].cumulative.clone()
        };
        let grid_local_rank = block_local_rank - start;
        let entry = &grids[j];

        (grid_local_rank, entry.multiplicity, &entry.fiber_size)
    }
    /// Returns the grid index for `multiplicity` in block `(c, k)` of
    /// child-pruned model `i`.
    fn grid_index_for_multiplicity(
        &self,
        i: usize,
        c: usize,
        k: usize,
        multiplicity: usize,
    ) -> Option<usize> {
        let grids = self.get(i, c, k);
        grids
            .binary_search_by_key(&multiplicity, |g| g.multiplicity)
            .ok()
    }

    fn build(
        f: &Feature,
        cfm: &CFM,
        dp: &CountDpTable,
        total_config_counts: &FeatureVec<Integer>,
    ) -> Self {
        let children = cfm.children(f);
        let n_children = children.len();

        let max_c = max_card(cfm.group_type_cardinality(f));
        let max_k = max_card(cfm.group_instance_cardinality(f));

        let blocks_per_model = (max_c + 1) * (max_k + 1);
        let total_slots = n_children * blocks_per_model;

        let sizes = vec![CumGridEntry { grids: Vec::new() }; total_slots];

        let mut grid_sizes = Self {
            max_c,
            max_k,
            sizes,
        };

        let stride = max_k + 1;
        for (i0, child) in children.enumerate() {
            let i = i0 + 1; // convert to 1-based model index

            let child_total = &total_config_counts[child];

            // Precompute fiber sizes
            let rs: Vec<usize> = cfm
                .feature_instance_cardinality(child)
                .into_iter()
                .collect();
            let fiber_sizes = Multiset::count_multisets_batched(child_total, &rs);

            let fiber_sizes: Vec<(usize, Arc<Integer>)> = rs
                .into_iter()
                .zip(fiber_sizes.into_iter())
                .filter_map(|(r, val)| {
                    if val.is_zero() {
                        None
                    } else {
                        Some((r, Arc::new(val)))
                    }
                })
                .collect();

            // FIRST CHILD (i = 0) — direct initialization
            if i0 == 0 {
                let model_base = grid_sizes.model_base(i);

                for &(r, ref fiber_size) in &fiber_sizes {
                    let c = usize::from(r > 0);
                    let k = r;

                    if c > max_c || k > max_k {
                        break;
                    }

                    let flat = c * stride + k;

                    grid_sizes.sizes[model_base + flat] = CumGridEntry {
                        grids: vec![GridEntry {
                            cumulative: fiber_size.as_ref().clone(),
                            multiplicity: r,
                            fiber_size: fiber_size.clone(),
                        }],
                    };
                }

                continue;
            }

            // i0 would be wrong since we backtrack in contrast to dp which is forward
            for c in 0..=max_c.min(i) {
                for k in c..=max_k {
                    let mut cumulative = Vec::new();
                    let mut acc = Integer::from(0);
                    // Backward (pull) transition
                    for &(r, ref fiber_size) in &fiber_sizes {
                        if r > k {
                            break;
                        }

                        let ind = usize::from(r > 0);
                        if ind > c {
                            break;
                        }
                        let c_prev = c - ind;
                        let k_prev = k - r;
                        let base_count = dp.get(i0, c_prev, k_prev);
                        if base_count.is_zero() {
                            continue;
                        }
                        acc += base_count * fiber_size.as_ref();
                        cumulative.push(GridEntry {
                            cumulative: acc.clone(),
                            multiplicity: r,
                            fiber_size: fiber_size.clone(),
                        });
                    }
                    if !cumulative.is_empty() {
                        let flat = c * stride + k;
                        let idx = grid_sizes.model_base(i) + flat;
                        grid_sizes.sizes[idx] = CumGridEntry { grids: cumulative };
                    }
                }
            }
        }
        grid_sizes
    }

    /// Iterates over all blocks and yields the grid slice for each block.
    ///
    /// # Iteration order
    ///
    /// Blocks are visited in the following deterministic order:
    ///
    /// 1. Child-pruned model index `i` in increasing order (1-based).
    /// 2. Within each model, blocks are ordered by increasing `c`
    ///    from `0..=max_c`.
    /// 3. For a fixed `c`, blocks are ordered by increasing `k`
    ///    from `0..=max_k`.
    ///
    /// Therefore the logical order is:
    ///
    /// ```text
    /// for i in 1..=n_children {
    ///     for c in 0..=max_c {
    ///         for k in 0..=max_k {
    ///             yield grids(i, c, k)
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// Each yielded slice contains [`GridEntry`] values sorted by
    /// increasing `multiplicity`.
    ///
    /// For a grid entry `j`:
    /// - `entry.cumulative` is the cumulative size up to and including this grid.
    /// - `entry.multiplicity` is the child multiplicity `r`.
    /// - `entry.fiber_size` is the number of multisets for this multiplicity.
    pub fn iter_blocks_with_grids(&self) -> impl Iterator<Item = &[GridEntry]> + '_ {
        self.sizes.iter().map(|entry| entry.grids.as_slice())
    }
}

impl ConfigSpace for StructuralConfigSpace {
    type Config = StructuralConfiguration;
    type Cache = StructuralCache;

    fn build_cache(&self) -> Self::Cache {
        let dp_cache = self.build_dp_cache();

        let n_features = self.cfm.size();

        let dummy_block = CumBlockSizes { blocks: Vec::new() };

        let dummy_grid = CumGridSizes {
            max_c: 0,
            max_k: 0,
            sizes: Vec::new(),
        };

        // build cumulative block sizes and grid sizes
        let mut cum_block_sizes: FeatureVec<CumBlockSizes> =
            FeatureVec::new(vec![dummy_block; n_features]);

        let mut cum_grid_sizes: FeatureVec<CumGridSizes> =
            FeatureVec::new(vec![dummy_grid; n_features]);

        let mut child_positions: FeatureVec<Option<usize>> =
            FeatureVec::new(vec![None; n_features]);

        for feature in self.cfm.pre_order() {
            let block =
                CumBlockSizes::build(feature, &self.cfm, &dp_cache.count_dp_tables()[feature]);
            cum_block_sizes[feature] = block;
            let grid = CumGridSizes::build(
                feature,
                &self.cfm,
                &dp_cache.count_dp_tables()[feature],
                dp_cache.total_config_counts(),
            );
            cum_grid_sizes[feature] = grid;

            for (i, &child) in self.cfm.children(feature).enumerate() {
                child_positions[child] = Some(i);
            }
        }

        StructuralCache {
            dp_cache,
            cum_block_sizes,
            cum_grid_sizes,
            child_positions,
        }
    }

    fn count(&self, cache: &Self::Cache) -> Integer {
        let root = self.cfm.root();
        cache.dp_cache.total_config_counts[root].clone()
    }

    fn rank(&self, cache: &Self::Cache, config: &Self::Config) -> Integer {
        cache.rank_configuration(&self.cfm, config)
    }

    fn unrank(&self, cache: &Self::Cache, rank: &Integer) -> Self::Config {
        cache.unrank_configuration(&self.cfm, rank)
    }
}

#[derive(Clone)]
pub struct ConfigurationCursor {
    rank: Integer,
    c: usize,
    k: usize,
    feature: Feature,
    children: Vec<ChildCursor>,
    feature_counts: FeatureVec<usize>,
}

impl ConfigurationCursor {
    #[inline]
    #[must_use]
    pub fn feature_counts(&self) -> &FeatureVec<usize> {
        &self.feature_counts
    }
}

#[derive(Clone)]
struct ChildCursor {
    child_feature: Feature,
    sub_cursors: Vec<ConfigurationCursor>,
}

impl StructuralConfigSpace {
    #[must_use]
    pub fn unrank_into_cursor(
        &self,
        cache: &StructuralCache,
        rank: &Integer,
    ) -> ConfigurationCursor {
        cache.unrank_cursor(&self.cfm, rank)
    }

    #[must_use]
    pub fn build_from_cursor(
        &self,
        cache: &StructuralCache,
        cursor: &ConfigurationCursor,
    ) -> StructuralConfiguration {
        cache.build_configuration_from_cursor(&self.cfm, cursor)
    }

    pub fn increment_cursor(
        &self,
        cache: &StructuralCache,
        cursor: &mut ConfigurationCursor,
    ) -> bool {
        cache.next_node(&self.cfm, cursor)
    }
}

#[must_use]
pub fn max_card(card: &CardinalityInterval) -> usize {
    match card.max() {
        Some(m) => m,
        None => panic!("Unbounded cardinality encountered in DP."),
    }
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use super::*;
    use serde_json::{Value, json};

    use crate::{
        config_spaces::{ConfigSpace, Configuration, structural::StructuralConfigSpace},
        model::cfm::CFM,
        test_cfms::TestCFM,
        utils::data_structures::Tree,
    };

    #[test]
    fn ranking_basic() {
        let cfm = TestCFM::build_simple_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());

        let cache = space.build_cache();

        // count
        let count: usize = space.count(&cache).try_into().unwrap();
        assert_eq!(count, 4, "Expected exactly 4 configurations");

        // unrank all configurations
        let mut json_configs: Vec<Value> = Vec::new();

        for r in 0..count {
            let cfg = space.unrank(&cache, &Integer::from(r));
            let json = cfg.serialize(&cfm);
            json_configs.push(json);
        }

        // ensure all JSON representations are unique
        for i in 0..json_configs.len() {
            for j in (i + 1)..json_configs.len() {
                assert_ne!(
                    json_configs[i], json_configs[j],
                    "Duplicate configuration JSON at ranks {i} and {j}"
                );
            }
        }

        // rank - unrank consistency
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let r_back = space.rank(&cache, &cfg);

            assert_eq!(r, r_back, "rank(unrank({r})) returned {r_back}");
        }

        // compare against hand-enumerated JSON configurations

        let expected: Vec<Value> = vec![
            // 1) Root_1
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": []
            }),
            // 2) Root_1 -> A_1
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": [
                    {
                        "name": "A_1",
                        "feature": "A",
                        "instance": 1,
                        "children": []
                    }
                ]
            }),
            // 3) Root_1 -> A_1, B_1
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": [
                    {
                        "name": "A_1",
                        "feature": "A",
                        "instance": 1,
                        "children": []
                    },
                    {
                        "name": "B_1",
                        "feature": "B",
                        "instance": 1,
                        "children": []
                    }
                ]
            }),
            // 4) Root_1 -> B_1
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": [
                    {
                        "name": "B_1",
                        "feature": "B",
                        "instance": 1,
                        "children": []
                    }
                ]
            }),
        ];

        for e in &expected {
            assert!(
                json_configs.contains(e),
                "Expected configuration not found:\n{e:#}"
            );
        }

        assert_eq!(
            json_configs.len(),
            expected.len(),
            "Unexpected number of configurations"
        );
    }

    #[test]
    fn ranking_wide() {
        let cfm = TestCFM::build_wide_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: usize = space.count(&cache).try_into().unwrap();
        assert_eq!(count, 27, "Expected exactly 27 configurations");

        // Unrank all configurations
        let mut json_configs: Vec<Value> = Vec::new();

        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let json = cfg.serialize(&cfm);
            json_configs.push(json);
        }

        // Expected configurations
        fn child(feature: &str, idx: usize) -> Value {
            json!({
                "name": format!("{feature}_{idx}"),
                "feature": feature,
                "instance": idx,
                "children": []
            })
        }

        fn root(children: Vec<Value>) -> Value {
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": children
            })
        }

        let mut expected: Vec<Value> = Vec::new();

        // multiplicities: A ∈ {0,1,2}, B ∈ {0,1,2}, C ∈ {0,1,2}
        for a_mult in 0..=2 {
            for b_mult in 0..=2 {
                for c_mult in 0..=2 {
                    let mut children = Vec::new();

                    for i in 1..=a_mult {
                        children.push(child("A", i));
                    }
                    for i in 1..=b_mult {
                        children.push(child("B", i));
                    }
                    for i in 1..=c_mult {
                        children.push(child("C", i));
                    }

                    expected.push(root(children));
                }
            }
        }

        assert_eq!(expected.len(), 27);

        // Ensure enumeration matches expected JSON exactly
        for e in &expected {
            assert!(
                json_configs.contains(e),
                "Expected configuration not found:\n{e:#}"
            );
        }

        assert_eq!(
            json_configs.len(),
            expected.len(),
            "Unexpected number of configurations"
        );

        // Rank / unrank roundtrip
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let r_back = space.rank(&cache, &cfg);
            assert_eq!(r, r_back, "rank(unrank({r})) returned {r_back}");

            let cfg2 = space.unrank(&cache, &r);
            let json1 = cfg.serialize(&cfm);
            let json2 = cfg2.serialize(&cfm);
            assert_eq!(json1, json2, "unrank(rank(cfg)) changed configuration");
        }
    }
    #[test]
    fn ranking_deep() {
        let cfm = TestCFM::build_deep_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: usize = space.count(&cache).try_into().unwrap();
        // Rank / unrank roundtrip
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let r_back = space.rank(&cache, &cfg);
            assert_eq!(r, r_back, "rank(unrank({r})) returned {r_back}");

            let cfg2 = space.unrank(&cache, &r_back);
            let json1 = cfg.serialize(&cfm);
            let json2 = cfg2.serialize(&cfm);
            assert_eq!(json1, json2, "unrank(rank(cfg)) changed configuration");
        }
    }

    #[test]
    fn ranking_with_gaps() {
        let cfm = TestCFM::build_gap_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: usize = space.count(&cache).try_into().unwrap();
        assert_eq!(count, 3, "Expected exactly 3 configurations");

        let mut json_configs: Vec<Value> = Vec::new();

        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let json = cfg.serialize(&cfm);
            json_configs.push(json);
        }

        let expected: Vec<Value> = vec![
            // 1) Root_1
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": []
            }),
            // 2) Root_1 -> A_1, A_2
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": [
                    {
                        "name": "A_1",
                        "feature": "A",
                        "instance": 1,
                        "children": []
                    },
                    {
                        "name": "A_2",
                        "feature": "A",
                        "instance": 2,
                        "children": []
                    }
                ]
            }),
            // 3) Root_1 -> B_1, B_2
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": [
                    {
                        "name": "B_1",
                        "feature": "B",
                        "instance": 1,
                        "children": []
                    },
                    {
                        "name": "B_2",
                        "feature": "B",
                        "instance": 2,
                        "children": []
                    }
                ]
            }),
        ];

        for e in &expected {
            assert!(
                json_configs.contains(e),
                "Expected configuration not found:\n{e:#}"
            );
        }

        assert_eq!(
            json_configs.len(),
            expected.len(),
            "Unexpected number of configurations"
        );

        // Rank / unrank roundtrip
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let r_back = space.rank(&cache, &cfg);
            assert_eq!(r, r_back, "rank(unrank({r})) returned {r_back}");

            let cfg2 = space.unrank(&cache, &r_back);
            assert_eq!(
                cfg.serialize(&cfm),
                cfg2.serialize(&cfm),
                "unrank(rank(cfg)) changed configuration"
            );
        }
    }

    #[test]
    fn ranking_with_large_gaps() {
        let cfm = TestCFM::build_large_gap_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: usize = space.count(&cache).try_into().unwrap();

        // {0} + {A only: 3} + {B only: 3} + {A,B: 3x3}
        assert_eq!(count, 16, "Expected exactly 16 configurations");

        let mut json_configs: Vec<Value> = Vec::new();
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let json = cfg.serialize(&cfm);
            json_configs.push(json);
        }
        // Helper to build children
        fn mk_children(feature: &str, n: usize) -> Vec<Value> {
            (1..=n)
                .map(|i| {
                    json!({
                        "name": format!("{feature}_{i}"),
                        "feature": feature,
                        "instance": i,
                        "children": []
                    })
                })
                .collect()
        }

        fn root(children: Vec<Value>) -> Value {
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": children
            })
        }

        let allowed = [5usize, 10, 1000];

        let mut expected: Vec<Value> = Vec::new();

        // Root only
        expected.push(root(vec![]));

        // A only
        for &a in &allowed {
            expected.push(root(mk_children("A", a)));
        }

        // B only
        for &b in &allowed {
            expected.push(root(mk_children("B", b)));
        }

        // A + B
        for &a in &allowed {
            for &b in &allowed {
                let mut children = Vec::new();
                children.extend(mk_children("A", a));
                children.extend(mk_children("B", b));
                expected.push(root(children));
            }
        }

        assert_eq!(expected.len(), 16);

        // Ensure all expected configurations appear
        for e in &expected {
            assert!(
                json_configs.contains(e),
                "Expected configuration not found:\n{e:#}"
            );
        }

        assert_eq!(
            json_configs.len(),
            expected.len(),
            "Unexpected number of configurations"
        );

        // Rank / unrank roundtrip
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let r_back = space.rank(&cache, &cfg);
            assert_eq!(r, r_back, "rank(unrank({r})) returned {r_back}");

            let cfg2 = space.unrank(&cache, &r_back);
            assert_eq!(
                cfg.serialize(&cfm),
                cfg2.serialize(&cfm),
                "unrank(rank(cfg)) changed configuration"
            );
        }
    }

    #[test]
    fn ranking_with_cutoff_branch() {
        let cfm = TestCFM::build_cutoff_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: usize = space.count(&cache).try_into().unwrap();
        assert_eq!(count, 2, "Expected exactly 2 configurations");

        let mut json_configs: Vec<Value> = Vec::new();
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            json_configs.push(cfg.serialize(&cfm));
        }

        let expected: Vec<Value> = vec![
            // 1) Root_1
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": []
            }),
            // 2) Root_1 -> A_1
            json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": [
                    {
                        "name": "A_1",
                        "feature": "A",
                        "instance": 1,
                        "children": []
                    }
                ]
            }),
        ];

        for e in &expected {
            assert!(
                json_configs.contains(e),
                "Expected configuration not found:\n{e:#}"
            );
        }

        assert_eq!(json_configs.len(), expected.len());

        // Rank / unrank roundtrip
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let r_back = space.rank(&cache, &cfg);
            assert_eq!(r, r_back);
        }
    }
    #[test]
    fn ranking_deep_chain_with_wide_leaf() {
        let cfm = TestCFM::build_deep_chain_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: usize = space.count(&cache).try_into().unwrap();
        assert_eq!(count, 8, "Expected exactly 8 configurations");

        let mut json_configs: Vec<Value> = Vec::new();
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            json_configs.push(cfg.serialize(&cfm));
        }

        // Enumerate all subsets of {X,Y,Z}
        fn leaf(name: &str) -> Value {
            json!({
                "name": format!("{name}_1"),
                "feature": name,
                "instance": 1,
                "children": []
            })
        }

        let mut expected: Vec<Value> = Vec::new();
        for mask in 0..8 {
            let mut leaves = Vec::new();
            if mask & 1 != 0 {
                leaves.push(leaf("X"));
            }
            if mask & 2 != 0 {
                leaves.push(leaf("Y"));
            }
            if mask & 4 != 0 {
                leaves.push(leaf("Z"));
            }

            let cfg = json!({
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": [{
                    "name": "A_1",
                    "feature": "A",
                    "instance": 1,
                    "children": [{
                        "name": "B_1",
                        "feature": "B",
                        "instance": 1,
                        "children": [{
                            "name": "C_1",
                            "feature": "C",
                            "instance": 1,
                            "children": [{
                                "name": "D_1",
                                "feature": "D",
                                "instance": 1,
                                "children": [{
                                    "name": "LeafRoot_1",
                                    "feature": "LeafRoot",
                                    "instance": 1,
                                    "children": leaves
                                }]
                            }]
                        }]
                    }]
                }]
            });

            expected.push(cfg);
        }

        assert_eq!(expected.len(), 8);

        for e in &expected {
            assert!(
                json_configs.contains(e),
                "Expected configuration not found:\n{e:#}"
            );
        }

        // Rank / unrank roundtrip
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let r_back = space.rank(&cache, &cfg);
            assert_eq!(r, r_back);
        }
    }

    #[test]
    fn ranking_group_restricts_large_multiplicity() {
        let cfm = TestCFM::build_group_restricted_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: usize = space.count(&cache).try_into().unwrap();
        assert_eq!(count, 5, "Expected exactly 5 configurations");

        let mut json_configs: Vec<Value> = Vec::new();
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            json_configs.push(cfg.serialize(&cfm));
        }

        fn child(feature: &str, n: usize) -> Vec<Value> {
            (1..=n)
                .map(|i| {
                    json!( {
                        "name": format!("{feature}_{i}"),
                        "feature": feature,
                        "instance": i,
                        "children": []
                    })
                })
                .collect()
        }

        fn root(children: Vec<Value>) -> Value {
            json!( {
                "name": "Root_1",
                "feature": "Root",
                "instance": 1,
                "children": [{
                    "name": "A_1",
                    "feature": "A",
                    "instance": 1,
                    "children": children
                }]
            })
        }

        // A: type ∈ [0..1] and instance ∈ [0..2]
        // => either choose X or choose Y (or none), but never both.
        let expected: Vec<Value> = vec![
            root(vec![]),        // none
            root(child("X", 1)), // X1
            root(child("X", 2)), // X2
            root(child("Y", 1)), // Y1
            root(child("Y", 2)), // Y2
        ];

        assert_eq!(expected.len(), 5);

        for e in &expected {
            assert!(
                json_configs.contains(e),
                "Expected configuration not found:\n{e:#}"
            );
        }

        assert_eq!(
            json_configs.len(),
            expected.len(),
            "Unexpected number of configurations"
        );

        // Rank / unrank roundtrip
        for r in 0..count {
            let r = Integer::from(r);
            let cfg = space.unrank(&cache, &r);
            let r_back = space.rank(&cache, &cfg);
            assert_eq!(r, r_back, "rank(unrank({r})) returned {r_back}");
        }
    }

    #[test]
    fn ranking_dead_branch_cfm() {
        let cfm = TestCFM::build_dead_branch_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: i32 = space.count(&cache).try_into().unwrap();

        // A is optional but its subtree is dead, so A can never appear.
        // B is mandatory.
        //
        // Therefore exactly ONE configuration exists:
        //
        //   Root_1
        //     └── B_1
        //
        assert_eq!(count, 1, "Expected exactly 1 configuration");

        let cfg = space.unrank(&cache, &Integer::from(0));
        let json_cfg = cfg.serialize(&cfm);

        let expected = json!({
            "name": "Root_1",
            "feature": "Root",
            "instance": 1,
            "children": [
                {
                    "name": "B_1",
                    "feature": "B",
                    "instance": 1,
                    "children": []
                }
            ]
        });

        assert_eq!(
            json_cfg, expected,
            "Dead-branch CFM produced unexpected configuration"
        );

        // Rank / unrank roundtrip consistency
        let r_back = space.rank(&cache, &cfg);
        assert_eq!(r_back, Integer::from(0), "rank(unrank(0)) must be 0");

        let cfg2 = space.unrank(&cache, &r_back);
        assert_eq!(
            cfg.serialize(&cfm),
            cfg2.serialize(&cfm),
            "unrank(rank(cfg)) changed configuration"
        );
    }

    #[test]
    fn ranking_double_invalid_cutoff() {
        let cfm = TestCFM::build_double_invalid_cutoff_cfm();
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: usize = space.count(&cache).try_into().unwrap();
        assert_eq!(count, 1, "Expected exactly 1 configuration");

        let cfg = space.unrank(&cache, &Integer::from(0));
        let json_cfg = cfg.serialize(&cfm);

        let expected = json!({
            "name": "Root_1",
            "feature": "Root",
            "instance": 1,
            "children": []
        });

        assert_eq!(
            json_cfg, expected,
            "Double-invalid cutoff CFM produced unexpected configuration"
        );

        // Rank / unrank roundtrip
        let r_back = space.rank(&cache, &cfg);
        assert_eq!(r_back, Integer::from(0), "rank(unrank(0)) must be 0");

        let cfg2 = space.unrank(&cache, &r_back);
        assert_eq!(
            cfg.serialize(&cfm),
            cfg2.serialize(&cfm),
            "unrank(rank(cfg)) changed configuration"
        );
    }

    fn assert_cursor_consistency(cfm: Arc<CFM>) {
        let space = StructuralConfigSpace::new(cfm.clone());
        let cache = space.build_cache();

        let count: usize = space.count(&cache).try_into().unwrap();

        // --- 1) Direct unrank vs build_from_cursor(unrank_cursor)
        for i in 0..count {
            let rank = Integer::from(i);

            let cfg_direct = space.unrank(&cache, &rank);

            let cursor = space.unrank_into_cursor(&cache, &rank);
            let cfg_from_cursor = space.build_from_cursor(&cache, &cursor);

            assert_eq!(
                cfg_direct.serialize(&cfm),
                cfg_from_cursor.serialize(&cfm),
                "unrank vs cursor-build mismatch at rank {rank}"
            );
        }

        // --- 2) Incremental cursor enumeration
        let mut cursor = space.unrank_into_cursor(&cache, &Integer::from(0));

        for i in 0..count {
            let expected_rank = Integer::from(i);

            // rank must match loop index
            assert_eq!(
                cursor.rank, expected_rank,
                "cursor rank desynced before step {i}"
            );

            let cfg_expected = space.unrank(&cache, &cursor.rank);
            let cfg_from_inc = space.build_from_cursor(&cache, &cursor);

            assert_eq!(
                cfg_expected.serialize(&cfm),
                cfg_from_inc.serialize(&cfm),
                "incremental cursor mismatch at rank {}",
                cursor.rank
            );

            // feature count check
            assert_eq!(
                cursor.feature_counts,
                *cfg_expected.feature_counts(&cfm),
                "global feature counts mismatch (incremental) at rank {}",
                cursor.rank
            );

            assert_eq!(
                cursor.feature_counts().iter().sum::<usize>(),
                cfg_expected.size()
            );

            // Advance cursor unless we're at the last element
            if i + 1 < count {
                let ok = space.increment_cursor(&cache, &mut cursor);
                assert!(
                    ok,
                    "increment returned false before reaching last rank (at rank {i})"
                );
            }
        }

        // --- 3) Ensure increment properly reports exhaustion
        let exhausted = space.increment_cursor(&cache, &mut cursor);
        assert!(
            !exhausted,
            "increment should return false after final configuration"
        );

        // rank must not advance past the last valid value
        assert_eq!(
            cursor.rank,
            Integer::from(count - 1),
            "cursor rank advanced past final configuration"
        );
    }

    #[test]
    fn cursor_consistency_simple() {
        assert_cursor_consistency(TestCFM::build_simple_cfm());
    }

    #[test]
    fn cursor_consistency_wide() {
        assert_cursor_consistency(TestCFM::build_wide_cfm());
    }

    #[test]
    fn cursor_consistency_deep() {
        assert_cursor_consistency(TestCFM::build_deep_cfm());
    }

    #[test]
    fn cursor_consistency_with_gaps() {
        assert_cursor_consistency(TestCFM::build_gap_cfm());
    }

    #[test]
    fn cursor_consistency_with_large_gaps() {
        assert_cursor_consistency(TestCFM::build_large_gap_cfm());
    }

    #[test]
    fn cursor_consistency_cutoff_branch() {
        assert_cursor_consistency(TestCFM::build_cutoff_cfm());
    }

    #[test]
    fn cursor_consistency_deep_chain_with_wide_leaf() {
        assert_cursor_consistency(TestCFM::build_deep_chain_cfm());
    }

    #[test]
    fn cursor_consistency_group_restricted() {
        assert_cursor_consistency(TestCFM::build_group_restricted_cfm());
    }

    #[test]
    fn cursor_consistency_dead_branch() {
        assert_cursor_consistency(TestCFM::build_dead_branch_cfm());
    }

    #[test]
    fn cursor_consistency_double_invalid_cutoff() {
        assert_cursor_consistency(TestCFM::build_double_invalid_cutoff_cfm());
    }
}

use core::fmt;

use rug::{Complete, Integer, Rational};

use crate::{
    combinatorics::multiset::Multiset,
    config_spaces::structural::max_card,
    model::{
        cfm::CFM,
        feature::{Feature, FeatureVec},
    },
    utils::data_structures::{Tree, TreeTraversal as _},
};

pub type CountDpTable = DpTable<Integer>;

/// Flat DP table storing the dp values for `(i, c, k)` in a single vector.
///
/// Logical shape:
/// ```text
/// dp[i][c][k], where
///   i ∈ [0, i_max), c ∈ [0, c_max), k ∈ [0, k_max)
/// ```
///
/// Memory layout:
/// All `(c, k)` values for a fixed `i` are stored contiguously.
///
/// Index mapping:
/// ```text
/// index = (i * c_max + c) * k_max + k
/// ```
///
#[derive(Clone, Debug)]
pub struct DpTable<T> {
    i_len: usize,
    c_len: usize,
    k_len: usize,
    values: Vec<T>,
}

impl<T: Clone> DpTable<T> {
    pub fn new(i_len: usize, c_len: usize, k_len: usize, init: T) -> Self {
        let layer_size = c_len * k_len;
        let total = i_len * layer_size;

        Self {
            i_len,
            c_len,
            k_len,
            values: vec![init; total],
        }
    }

    #[inline]
    fn idx(&self, i: usize, c: usize, k: usize) -> usize {
        debug_assert!(i < self.i_len);
        debug_assert!(c < self.c_len);
        debug_assert!(k < self.k_len);
        (i * self.c_len + c) * self.k_len + k
    }

    #[inline]
    pub fn get(&self, i: usize, c: usize, k: usize) -> &T {
        &self.values[self.idx(i, c, k)]
    }

    #[inline]
    pub fn get_mut(&mut self, i: usize, c: usize, k: usize) -> &mut T {
        let idx = self.idx(i, c, k);
        &mut self.values[idx]
    }
}

impl<T: fmt::Display + Clone> fmt::Display for DpTable<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.i_len {
            writeln!(f, "===== i = {i} =====")?;

            // Header
            write!(f, "c\\k |")?;
            for k in 0..self.k_len {
                write!(f, " {k:>6}")?;
            }
            writeln!(f)?;
            writeln!(f, "{}", "-".repeat(7 + self.k_len * 7))?;

            for c in 0..self.c_len {
                write!(f, "{c:>3} |")?;
                for k in 0..self.k_len {
                    let v = self.get(i, c, k);
                    write!(f, " {v:>6}")?;
                }
                writeln!(f)?;
            }

            writeln!(f)?;
        }
        Ok(())
    }
}

pub fn compute_count_dp_tables(cfm: &CFM) -> (FeatureVec<CountDpTable>, FeatureVec<Integer>) {
    let n_features = cfm.size();

    let mut dp_tables: FeatureVec<Option<CountDpTable>> = vec![None; n_features].into();
    let mut total_configuration_counts: FeatureVec<Integer> =
        vec![Integer::from(0); n_features].into();

    for f in cfm.post_order() {
        let children: Vec<&Feature> = cfm.children(f).collect();

        // --------------------
        // Leaf
        // --------------------
        if children.is_empty() {
            // Shape (1, 1, 1)
            let mut table = DpTable::new(1, 1, 1, Integer::from(0));
            *table.get_mut(0, 0, 0) = Integer::from(1);

            dp_tables[f] = Some(table);
            total_configuration_counts[f] = Integer::from(1);
            continue;
        }

        // --------------------
        // Internal node
        // --------------------
        let c_max: usize = max_card(cfm.group_type_cardinality(f));
        let k_max: usize = max_card(cfm.group_instance_cardinality(f));
        let n = children.len();

        let child_multiplicities: Vec<Vec<usize>> = children
            .iter()
            .map(|child| {
                cfm.feature_instance_cardinality(child)
                    .into_iter()
                    .collect()
            })
            .collect();

        let mut current_table = DpTable::new(n + 1, c_max + 1, k_max + 1, Integer::from(0));

        // Base case
        *current_table.get_mut(0, 0, 0) = Integer::from(1);

        // FIRST CHILD (i = 0) — direct initialization
        {
            let child = children[0];
            let total_child = &total_configuration_counts[child];

            let rs = &child_multiplicities[0];
            let weights = Multiset::count_multisets_batched(total_child, rs);

            for (&r, weight) in rs.iter().zip(weights.into_iter()) {
                let c_rem = usize::from(r > 0);
                let k_rem = r;

                if k_rem > k_max || c_rem > c_max {
                    break; // r sorted -> all further r even larger
                }

                *current_table.get_mut(1, c_rem, k_rem) += weight;
            }
        }

        // compute prefix bounds
        let mut max_k_prefix = Vec::with_capacity(children.len() + 1);
        max_k_prefix.push(0);

        for mults in &child_multiplicities {
            let max_r = *mults.last().unwrap();
            let next = max_k_prefix.last().unwrap() + max_r;
            max_k_prefix.push(next);
        }

        // Fill DP
        for (i, &child) in children.iter().enumerate().skip(1) {
            let num_child_configs = &total_configuration_counts[child];

            // Precompute multiset counts
            let rs = &child_multiplicities[i];
            let weights = Multiset::count_multisets_batched(num_child_configs, rs);

            let weighted_rs: Vec<(usize, Integer)> =
                rs.iter().copied().zip(weights.into_iter()).collect();

            for c_prev in 0..=c_max.min(i) {
                for k_prev in c_prev..=max_k_prefix[i].min(k_max) {
                    let base_count = {
                        let bc = current_table.get(i, c_prev, k_prev);
                        if bc.is_zero() {
                            continue;
                        }
                        bc.clone()
                    };

                    for &(r, ref weight) in &weighted_rs {
                        let k_rem = k_prev + r;
                        let c_rem = c_prev + usize::from(r > 0);

                        if k_rem > k_max || c_rem > c_max {
                            break; // r sorted -> all further r even larger
                        }

                        *current_table.get_mut(i + 1, c_rem, k_rem) += &base_count * weight;
                    }
                }
            }
        }

        // --------------------
        // Compute total[f]
        // --------------------
        let mut total_configuration_count = Integer::from(0);

        for c in cfm.group_type_cardinality(f) {
            for k in cfm.group_instance_cardinality(f) {
                total_configuration_count += current_table.get(n, c, k);
            }
        }

        dp_tables[f] = Some(current_table);
        total_configuration_counts[f] = total_configuration_count;
    }
    let dp_tables: FeatureVec<CountDpTable> =
        dp_tables.map(|opt| opt.expect("dp_tables: every feature must be initialized"));
    (dp_tables, total_configuration_counts)
}

pub fn compute_expected_config_sizes(
    cfm: &CFM,
    dp_tables: &FeatureVec<CountDpTable>,
    total_configs: &FeatureVec<Integer>,
) -> FeatureVec<Rational> {
    let mut avg_sizes: FeatureVec<Rational> = vec![Rational::from(0); cfm.size()].into();

    for f in cfm.post_order() {
        let children: Vec<&Feature> = cfm.children(f).collect();

        // --------------------
        // Leaf
        // --------------------
        if children.is_empty() {
            avg_sizes[f] = Rational::from(1);
            continue;
        }

        let total_cfg = &total_configs[f];

        // If this feature has no valid configurations, it is unreachable,
        // so concrete values does not matter.
        if total_cfg.is_zero() {
            avg_sizes[f] = Rational::from(0);
            continue;
        }

        // --------------------
        // Internal node
        // --------------------
        let count_dp = &dp_tables[f];

        let c_max = count_dp.c_len - 1;
        let k_max = count_dp.k_len - 1;

        let mut prev = vec![Rational::from(0); (c_max + 1) * (k_max + 1)];
        let mut curr = vec![Rational::from(0); (c_max + 1) * (k_max + 1)];

        // Helper index
        let stride = k_max + 1;
        let idx = |c: usize, k: usize| c * stride + k;

        // Precompute multiplicities
        let child_multiplicities: Vec<Vec<usize>> = children
            .iter()
            .map(|child| {
                cfm.feature_instance_cardinality(child)
                    .into_iter()
                    .collect()
            })
            .collect();

        // FIRST CHILD (i = 0) — direct initialization
        {
            let child = children[0];
            let child_total = &total_configs[child];
            let child_avg = &avg_sizes[child];

            let rs = &child_multiplicities[0];
            let weights = Multiset::count_multisets_batched(child_total, rs);

            for (&r, weight) in rs.iter().zip(weights.into_iter()) {
                let k_rem = r;
                let c_rem = usize::from(r > 0);

                if k_rem > k_max || c_rem > c_max {
                    break; // r sorted -> all further r even larger
                }

                if weight.is_zero() {
                    continue;
                }

                let mut contribution = child_avg.clone();
                contribution *= r;
                contribution *= weight;
                curr[idx(c_rem, k_rem)] += contribution;
            }
        }

        std::mem::swap(&mut prev, &mut curr);
        curr.fill(Rational::from(0));

        // compute prefix bounds
        let mut max_k_prefix = Vec::with_capacity(children.len() + 1);
        max_k_prefix.push(0);

        for mults in &child_multiplicities {
            let max_r = *mults.last().unwrap();
            let next = max_k_prefix.last().unwrap() + max_r;
            max_k_prefix.push(next);
        }

        for (i, &child) in children.iter().enumerate().skip(1) {
            let child_total = &total_configs[child];
            let child_avg = &avg_sizes[child];

            // Precompute per-r:
            // - weight: #multisets
            // - r_avg:  r * child_avg
            let rs = &child_multiplicities[i];
            let weights = Multiset::count_multisets_batched(child_total, rs);

            let weighted_rs: Vec<(usize, Integer, Rational)> = rs
                .iter()
                .copied()
                .zip(weights.into_iter())
                .map(|(r, w)| {
                    let r_avg = child_avg * r;
                    (r, w, r_avg.complete())
                })
                .collect();

            for c_prev in 0..=c_max.min(i) {
                for k_prev in c_prev..=max_k_prefix[i].min(k_max) {
                    let base_count = count_dp.get(i, c_prev, k_prev);
                    if base_count.is_zero() {
                        continue;
                    }

                    let base_size = &prev[idx(c_prev, k_prev)];

                    for &(r, ref weight, ref r_avg) in &weighted_rs {
                        let c_rem = c_prev + usize::from(r != 0);
                        let k_rem = k_prev + r;
                        if c_rem > c_max || k_rem > k_max {
                            break; // r sorted -> all further r even larger
                        }

                        let delta = base_count * r_avg;

                        curr[idx(c_rem, k_rem)] += (base_size + delta.complete()) * weight;
                    }
                }
            }
            // advance to next i-layer
            std::mem::swap(&mut prev, &mut curr);
            curr.fill(Rational::from(0));
        }

        // --------------------
        // Final aggregation
        // --------------------
        let mut weighted_sum = Rational::from(0);

        for c in cfm.group_type_cardinality(f) {
            for k in cfm.group_instance_cardinality(f) {
                weighted_sum += &prev[idx(c, k)];
            }
        }

        avg_sizes[f] = 1 + weighted_sum / total_cfg;
    }

    avg_sizes
}

use std::sync::Arc;

use rug::{Complete, Integer};

use crate::{
    combinatorics::multiset::Multiset,
    config_spaces::structural::{
        ConfigurationCursor, StructuralBuilder, StructuralCache, StructuralConfiguration,
        StructuralNode, space::ChildCursor,
    },
    model::{
        cfm::CFM,
        feature::{Feature, FeatureVec},
    },
    utils::data_structures::Tree,
};

impl StructuralCache {
    #[must_use]
    pub fn unrank_configuration(&self, cfm: &CFM, rank: &Integer) -> StructuralConfiguration {
        let mut builder = StructuralBuilder::new(cfm.size());
        let root = cfm.root();
        let root_group = self.unrank_recursively(cfm, &mut builder, root, rank);
        builder.finish(root_group)
    }

    fn unrank_recursively(
        &self,
        cfm: &CFM,
        builder: &mut StructuralBuilder,
        f: &Feature,
        global_rank: &Integer,
    ) -> Arc<StructuralNode> {
        // Step 1: choose (c,k) block
        let (mut block_local_rank, mut c_rem, mut k_rem) =
            self.cum_block_sizes[f].find_block(global_rank);

        let mut node_builder = builder.begin_node(f);

        // Step 2: backward unranking of multiplicities + child-multisets
        for (i0, child) in cfm.children(f).enumerate().rev() {
            let i = i0 + 1; // child pruned are one based

            // grid_local_rank = which (multiplicity, multiset) pair we are in
            // r_i = multiplicity chosen for this child
            let (grid_local_rank, r_i, fiber_size) =
                self.cum_grid_sizes[f].find_grid(i, c_rem, k_rem, &block_local_rank);

            // fiber = multiset of r_i child configurations
            let fiber_local_rank = (&grid_local_rank % fiber_size.as_ref()).complete(); // which multiset
            let fiber_index = &grid_local_rank / fiber_size.as_ref(); // remainder for earlier children
            block_local_rank = fiber_index.complete();

            let child_ranks: Vec<Integer> =
                Multiset::unrank(&self.total_config_counts()[child], r_i, &fiber_local_rank);
            let len = child_ranks.len();
            let mut i = 0;

            while i < len {
                // Count how many times this rank repeats (including the one we just took)
                let child_rank = &child_ranks[i];
                let mut multiplicity = 1usize;

                let mut j = i + 1;
                while j < len && &child_ranks[j] == child_rank {
                    multiplicity += 1;
                    j += 1;
                }

                let child_group = self.unrank_recursively(cfm, builder, child, child_rank);

                node_builder = node_builder.add_child(child_group, multiplicity);

                i = j;
            }

            c_rem -= usize::from(r_i > 0);
            k_rem -= r_i;
        }

        debug_assert_eq!(c_rem, 0);
        debug_assert_eq!(k_rem, 0);
        builder.finish_node(node_builder)
    }

    #[must_use]
    pub fn unrank_cursor(&self, cfm: &CFM, rank: &Integer) -> ConfigurationCursor {
        let root = cfm.root();
        self.unrank_cursor_rec(cfm, root, rank)
    }

    fn unrank_cursor_rec(
        &self,
        cfm: &CFM,
        f: &Feature,
        global_rank: &Integer,
    ) -> ConfigurationCursor {
        // Step 1: choose (c,k) block
        let (mut block_local_rank, c, k) = self.cum_block_sizes[f].find_block(global_rank);
        let mut feature_counts: FeatureVec<usize> = vec![0; cfm.size()].into();
        feature_counts[*f] += 1;

        let children_list = cfm.children(f);
        let n = children_list.len();

        let mut children = vec![None; n];

        let mut c_rem = c;
        let mut k_rem = k;

        // Step 2: backward unranking of multiplicities + child-multisets
        for (i0, child) in children_list.enumerate().rev() {
            let i = i0 + 1;

            let (grid_local_rank, r_i, fiber_size) =
                self.cum_grid_sizes[f].find_grid(i, c_rem, k_rem, &block_local_rank);

            let fiber_rank = (&grid_local_rank % fiber_size.as_ref()).complete();
            let fiber_index = &grid_local_rank / fiber_size.as_ref();
            block_local_rank = fiber_index.complete();

            let ranks = Multiset::unrank(&self.total_config_counts()[child], r_i, &fiber_rank);
            debug_assert_eq!(ranks.len(), r_i);

            // build sub-cursors (from scratch) for each element rank
            let mut subs = Vec::with_capacity(r_i);
            for r in &ranks {
                let sub = self.unrank_cursor_rec(cfm, child, r);

                // add sub-cursor feature counts to current counts
                for (f, v) in sub.feature_counts.enumerate() {
                    feature_counts[f] += v;
                }

                subs.push(sub);
            }

            children[i0] = Some(ChildCursor {
                child_feature: *child,
                sub_cursors: subs,
            });

            c_rem -= usize::from(r_i > 0);
            k_rem -= r_i;
        }

        ConfigurationCursor {
            rank: global_rank.clone(),
            c,
            k,
            feature: *f,
            children: children.into_iter().map(|x| x.unwrap()).collect(),
            feature_counts,
        }
    }

    pub fn next_node(&self, cfm: &CFM, node: &mut ConfigurationCursor) -> bool {
        let f = node.feature;
        let n = node.children.len();

        node.rank += Integer::from(1);
        if node.rank >= self.total_config_counts()[f] {
            node.rank -= Integer::from(1);
            return false;
        }

        // 1. Try to advance the fiber local rank
        for i in (0..n).rev() {
            if self.advance_fiber_local_rank(cfm, node, i) {
                return true;
            }
        }

        // 2. Try to advance the grid
        for i in 0..n {
            if self.advance_grid(cfm, node, i) {
                return true;
            }
        }

        // 3. move to next block
        *node = self.unrank_cursor_rec(cfm, &f, &node.rank);
        true
    }

    fn advance_grid(&self, cfm: &CFM, node: &mut ConfigurationCursor, child_index: usize) -> bool {
        let f = node.feature;
        let n = node.children.len();

        let c_total = node.c;
        let k_total = node.k;

        // -------------------------------------------------
        // Suffix realized root group cardinalities
        // -------------------------------------------------
        let mut c_suf = 0;
        let mut k_suf = 0;
        for ch in node.children.iter().skip(child_index + 1) {
            let r = ch.sub_cursors.len();
            if r > 0 {
                c_suf += 1;
                k_suf += r;
            }
        }

        let c_rem = c_total - c_suf;
        let k_rem = k_total - k_suf;

        // -------------------------------------------------
        // Advance grid at child_index (multiplicity change)
        // -------------------------------------------------
        let old_r = node.children[child_index].sub_cursors.len();
        let grids = self.cum_grid_sizes[f].get(child_index + 1, c_rem, k_rem);

        let cur = match grids.binary_search_by_key(&old_r, |g| g.multiplicity) {
            Ok(idx) => idx,
            Err(_) => panic!("invalid grid"),
        };

        let new_r = match grids.get(cur + 1) {
            Some(g) => g.multiplicity,
            None => return false,
        };

        // ----- change multiplicity at child_index -----
        {
            let ch = &mut node.children[child_index];

            // subtract old contribution
            for sub in &ch.sub_cursors {
                for (ff, v) in sub.feature_counts.enumerate() {
                    node.feature_counts[ff] -= v;
                }
            }

            ch.sub_cursors.clear();
            ch.sub_cursors.reserve(new_r);

            if new_r > 0 {
                let zero = Integer::from(0);
                let zero_cursor = self.unrank_cursor_rec(cfm, &ch.child_feature, &zero);
                for _ in 0..new_r {
                    ch.sub_cursors.push(zero_cursor.clone());
                }

                for (ff, v) in zero_cursor.feature_counts.enumerate() {
                    node.feature_counts[ff] += v * new_r;
                }
            }
        }

        // -------------------------------------------------
        // Reset suffix fibers ranks
        // -------------------------------------------------
        for j in child_index + 1..n {
            self.reset_fiber_local_rank(cfm, node, j);
        }

        // -------------------------------------------------
        // Rebuild prefix multiplicities (0..child_index-1), fibers = 0
        // -------------------------------------------------
        let mut c_left = c_rem - usize::from(new_r > 0);
        let mut k_left = k_rem - new_r;

        for j in (0..child_index).rev() {
            let grids_j = self.cum_grid_sizes[f].get(j + 1, c_left, k_left);
            let rj = grids_j[0].multiplicity;

            // overwrite multiplicity at j
            {
                let ch = &mut node.children[j];

                for sub in &ch.sub_cursors {
                    for (ff, v) in sub.feature_counts.enumerate() {
                        node.feature_counts[ff] -= v;
                    }
                }

                ch.sub_cursors.clear();
                ch.sub_cursors.reserve(rj);

                if rj > 0 {
                    let zero = Integer::from(0);
                    let zero_cursor = self.unrank_cursor_rec(cfm, &ch.child_feature, &zero);
                    for _ in 0..rj {
                        ch.sub_cursors.push(zero_cursor.clone());
                    }

                    for (ff, v) in zero_cursor.feature_counts.enumerate() {
                        node.feature_counts[ff] += v * rj;
                    }
                }
            }

            if rj > 0 {
                c_left -= 1;
            }
            k_left -= rj;
        }

        debug_assert_eq!(c_left, 0);
        debug_assert_eq!(k_left, 0);

        true
    }

    fn advance_fiber_local_rank(
        &self,
        cfm: &CFM,
        node: &mut ConfigurationCursor,
        child_index: usize,
    ) -> bool {
        let child = &mut node.children[child_index];
        let k = child.sub_cursors.len();
        if k == 0 {
            return false;
        }

        // current multiset of ranks (non-decreasing)
        let mut ranks: Vec<Integer> = child
            .sub_cursors
            .iter_mut()
            .map(|c| c.rank.clone())
            .collect();
        debug_assert!(ranks.is_sorted());

        // advance multiset
        let Some(pivot) =
            Multiset::next_multiset(&mut ranks, &self.total_config_counts()[child.child_feature])
        else {
            return false;
        };

        // subtract old child contribution from parent
        for sub in &child.sub_cursors {
            for (f, v) in sub.feature_counts.enumerate() {
                node.feature_counts[f] -= v;
            }
        }

        // reset cursors [0..pivot)
        for (sub_cursor, rank) in child.sub_cursors.iter_mut().zip(ranks.iter()).take(pivot) {
            *sub_cursor = self.unrank_cursor_rec(cfm, &child.child_feature, rank);
        }

        // advance pivot cursor
        {
            let sub = &mut child.sub_cursors[pivot];
            self.next_node(cfm, sub);
        }

        // add new child contribution back to parent
        for sub in &child.sub_cursors {
            for (f, v) in sub.feature_counts.enumerate() {
                node.feature_counts[f] += v;
            }
        }

        // reset all following children (suffix) to first multiset
        for j in child_index + 1..node.children.len() {
            self.reset_fiber_local_rank(cfm, node, j);
        }

        true
    }

    /// Resets the fiber-local rank of the given child to 0.
    ///
    /// Keeps the child’s multiplicity unchanged, but replaces all sub-cursors
    /// with rank-0 (zero) cursors and updates `feature_counts` accordingly.
    fn reset_fiber_local_rank(
        &self,
        cfm: &CFM,
        node: &mut ConfigurationCursor,
        child_index: usize,
    ) {
        let child = &mut node.children[child_index];
        let child_feature = child.child_feature;
        let multiplicity = child.sub_cursors.len();

        // subtract old contribution

        for sub in &child.sub_cursors {
            for (f, v) in sub.feature_counts.enumerate() {
                node.feature_counts[f] -= v;
            }
        }

        // rebuild with `multiplicity` zero cursors

        child.sub_cursors.clear();
        child.sub_cursors.reserve(multiplicity);

        if multiplicity > 0 {
            let zero = Integer::from(0);
            let zero_cursor = self.unrank_cursor_rec(cfm, &child_feature, &zero);

            for _ in 0..multiplicity {
                child.sub_cursors.push(zero_cursor.clone());
            }

            // add new contribution
            for (f, v) in zero_cursor.feature_counts.enumerate() {
                node.feature_counts[f] += v * multiplicity;
            }
        }
    }

    #[must_use]
    pub fn build_configuration_from_cursor(
        &self,
        cfm: &CFM,
        cur: &ConfigurationCursor,
    ) -> StructuralConfiguration {
        let mut builder = StructuralBuilder::new(cfm.size());
        let root_group = Self::build_node_from_cursor(&mut builder, cur);
        builder.finish(root_group)
    }

    fn build_node_from_cursor(
        builder: &mut StructuralBuilder,
        cur: &ConfigurationCursor,
    ) -> Arc<StructuralNode> {
        let mut nb = builder.begin_node(&cur.feature);

        for ch in &cur.children {
            // each element is one instance of the child feature with its own subtree cursor
            for sub in &ch.sub_cursors {
                let child_group = Self::build_node_from_cursor(builder, sub);
                nb = nb.add_child(child_group, 1);
            }
        }

        builder.finish_node(nb)
    }

    #[must_use]
    pub fn rank_configuration(&self, cfm: &CFM, config: &StructuralConfiguration) -> Integer {
        let root = config.root();
        self.rank_recursively(cfm, root)
    }

    fn rank_recursively(&self, cfm: &CFM, node: &StructuralNode) -> Integer {
        let f = node.feature();
        let children = cfm.children(f);
        let n = children.len();

        // Leaf: only one configuration, rank is always 0
        if n == 0 {
            return Integer::from(0);
        }

        // compute realized (c,k)
        let mut c = 0usize;
        let mut k = 0usize;
        let mut seen = vec![false; n];
        for entry in node.children() {
            let m = entry.multiplicity();
            if m == 0 {
                continue;
            }

            let child_feature = *entry.node().feature();
            let i = self.child_positions[child_feature].unwrap();

            k += m;

            if !seen[i] {
                seen[i] = true;
                c += 1;
            }
        }

        let mut global_rank = Integer::from(0);

        // Step 1: add block start
        global_rank += self.cum_block_sizes[f].prefix_before(c, k);

        // collect child multiplicities + ranks

        let mut child_ranks: Vec<Vec<Integer>> = vec![Vec::new(); n];

        for entry in node.children() {
            let child_feature = entry.node().feature();
            let i = self.child_positions[child_feature].expect("child is always non root");

            let subtree_rank = self.rank_recursively(cfm, entry.node());
            let mult = entry.multiplicity();

            child_ranks[i].extend(std::iter::repeat_n(subtree_rank, mult));
        }

        // finalize multiplicities + sort multisets
        for ranks in &mut child_ranks {
            ranks.sort_unstable();
        }

        // Step 2: forward accumulation
        // Precompute remaining cardinalities at the beginning of each i-th iteration (0..n-1)
        let mut rem_c: Vec<usize> = vec![0; n];
        let mut rem_k: Vec<usize> = vec![0; n];
        rem_c[n - 1] = c;
        rem_k[n - 1] = k;

        for i in (0..n - 1).rev() {
            rem_c[i] = rem_c[i + 1] - usize::from(!child_ranks[i + 1].is_empty());
            rem_k[i] = rem_k[i + 1] - child_ranks[i + 1].len();
        }

        // build block-local rank R_i forward.
        // pruned_rank is the rank inside the child-pruned model
        let mut pruned_rank: Integer = Integer::from(0);

        for (i, _child) in cfm.children(f).enumerate() {
            let c_rem = rem_c[i];
            let k_rem = rem_k[i];

            let r_i = child_ranks[i].len();

            let grid_index = self.cum_grid_sizes[f]
                .grid_index_for_multiplicity(i + 1, c_rem, k_rem, r_i)
                .expect("multiplicity must always be present");

            // grid offset inside the block; child pruned models are one based
            let grid_start = self.cum_grid_sizes[f].prefix_before(i + 1, c_rem, k_rem, grid_index);

            // fiber size
            let entry = &self.cum_grid_sizes[f].get(i + 1, c_rem, k_rem)[grid_index];
            let fiber_size = entry.fiber_size.as_ref();

            // fiber-local rank

            let fiber_rank = Multiset::rank(&child_ranks[i]);
            debug_assert!(fiber_rank < *fiber_size);

            pruned_rank = grid_start + pruned_rank * fiber_size + fiber_rank;
        }

        global_rank += pruned_rank;

        global_rank
    }
}

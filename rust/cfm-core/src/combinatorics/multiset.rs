use num_traits::Zero;
use rug::{Complete, Integer, integer::IntegerExt64};

use crate::combinatorics::{binomial::Binomial, combination::Combination};

pub struct Multiset;

impl Multiset {
    /// Number of k-multisets over n elements:
    #[must_use]
    pub fn count_multisets(n: &Integer, k: usize) -> Integer {
        // k == 0 → exactly one multiset (the empty one)
        if k.is_zero() {
            return Integer::from(1);
        }
        // n == 0 and k > 0 → impossible
        if n.is_zero() {
            return Integer::from(0);
        }
        let n_plus_k_minus_1 = n + (k - 1);
        Binomial::binom(&n_plus_k_minus_1.complete(), k)
    }

    /// Compute `count_multisets(n, k)` for a strictly increasing list of `k`s.
    ///
    /// - `ks` must be sorted and strictly increasing (no duplicates).
    /// - Returns a vector `out` where `out[i] = count_multisets(n, ks[i])`.
    ///
    /// For consecutive values of `k`, we use the identity
    /// C(n + t, t + 1) = C(n + t - 1, t) * (n + t) / (t + 1)
    ///
    /// starting from the previously computed value and stepping forward.
    #[must_use]
    pub fn count_multisets_batched(n: &Integer, ks: &[usize]) -> Vec<Integer> {
        let mut out = Vec::with_capacity(ks.len());

        if ks.is_empty() {
            return out;
        }

        // Compute first exactly
        let mut last_k = ks[0];
        let mut last_val = Self::count_multisets(n, last_k);
        out.push(last_val.clone());

        for &k in &ks[1..] {
            debug_assert!(k > last_k, "ks must be strictly increasing");

            for t in last_k..k {
                // We update:
                //   last_val *= (n + t) / (t + 1)
                //
                // g1 = gcd(n+t, t+1).
                // After dividing:
                //   numer = (n+t)/g1
                //   denom = (t+1)/g1
                // so numer and denom are coprime.
                //
                // g2 = gcd(last_val, denom).
                // Since last_val * numer / denom is known to be an integer
                // and numer, denom are coprime, denom must divide last_val.
                // Therefore g2 = denom, and after dividing:
                //
                //   last_val := last_val/g2
                //   denom    := denom/g2
                //
                // we have denom == 1 and
                //
                //   last_val *= (n+t)/(t+1)
                //            = (last_val/g2) * ((n+t)/g1)

                let mut numer = (n + t).complete();
                let mut denom = (t + 1) as u64;

                let g1: u64 = numer
                    .gcd_u64_ref(denom)
                    .complete()
                    .to_u64()
                    .expect("gcd is divisor of denom so fits into u64");

                numer /= g1;
                denom /= g1;

                let g2 = last_val
                    .gcd_u64_ref(denom)
                    .complete()
                    .to_u64()
                    .expect("gcd is divisor of denom so fits into u64");

                last_val /= g2;
                denom /= g2;

                debug_assert!(denom == 1);

                last_val *= numer;
            }

            out.push(last_val.clone());
            last_k = k;
        }

        out
    }

    /// Rank a k-multiset.
    ///
    /// Input:
    ///   - `multiset`: non-decreasing sequence (`m_1,...,m_k`)
    ///   - each `m_i` in {0,...,n-1}
    ///
    /// Output:
    ///   - Rank `R` in {0,..., binom(n+k-1, k)-1}
    #[must_use]
    pub fn rank(multiset: &[Integer]) -> Integer {
        let k = multiset.len();

        if k.is_zero() {
            return Integer::from(0);
        }

        let mut c = Vec::with_capacity(k);
        for (i, m_i) in multiset.iter().enumerate() {
            c.push((m_i + i).complete());
        }

        Combination::rank(&c)
    }

    /// Unrank a k-multiset.
    ///
    /// Input:
    ///   - `n`: Number of elements in the base set. Valid element values are `0 .. n-1`.
    ///   - `k`: size of the multiset
    ///   - `rank`: rank in {0,..., binom(n+k-1, k)-1}
    ///
    /// Output:
    ///   - non-decreasing sequence (`m_1,...,m_k`)
    #[must_use]
    pub fn unrank(n: &Integer, k: usize, rank: &Integer) -> Vec<Integer> {
        if k.is_zero() {
            return Vec::new();
        }

        let universe = n + (k - 1); // multiset is represented as shifted sequence
        let combination = Combination::unrank(&universe.complete(), k, rank);

        let mut multiset = Vec::with_capacity(k);
        for (i, c_i) in combination.into_iter().enumerate() {
            multiset.push(c_i - i);
        }
        multiset
    }

    /// Advance a k-multiset to the next value in colex order.
    ///
    /// On success, mutates `m` in place and returns `Some(pivot)`,
    /// where `pivot` is the lowest index whose value was increased
    /// (by 1); all indices `< pivot` are set to their minimal values.
    ///
    /// Returns `None` if `m` is already the last multiset.
    pub fn next_multiset<M>(multiset: &mut M, n: &Integer) -> Option<usize>
    where
        M: AsMut<[Integer]>,
    {
        let multiset = multiset.as_mut();
        let k = multiset.len();
        if k == 0 {
            return None;
        }

        if n.is_zero() {
            return None;
        }

        let n_minus_one = n.clone() - 1;

        for pivot in 0..k {
            let can_increment = if pivot + 1 < k {
                multiset[pivot] < multiset[pivot + 1]
            } else {
                multiset[pivot] < n_minus_one
            };

            if can_increment {
                // increment pivot
                multiset[pivot] += 1usize;

                // reset lower positions
                for x in multiset.iter_mut().take(pivot) {
                    *x = Integer::from(0);
                }

                return Some(pivot);
            }
        }

        None // already last multiset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_next_multiset_full(n: usize, k: usize) {
        let n = Integer::from(n);

        let total = Multiset::count_multisets(&n, k);

        // Enumerate by rank (ground truth)
        let mut expected = Vec::new();
        let mut r = Integer::from(0);
        while r < total {
            expected.push(Multiset::unrank(&n, k, &r));
            r += 1u32;
        }

        // Enumerate using next_multiset
        let mut current = expected.first().cloned().unwrap_or_else(Vec::new);
        let mut actual = vec![current.clone()];

        while let Some(pivot) = Multiset::next_multiset(&mut current, &n) {
            let prev = actual.last().unwrap().clone();

            // indices below pivot reset to minimal values
            for (i, v) in current.iter().take(pivot).enumerate() {
                assert_eq!(
                    *v,
                    Integer::from(0usize),
                    "index {i} not reset for n={n}, k={k}"
                );
            }

            // pivot increased by exactly 1
            if pivot < k {
                assert_eq!(
                    current[pivot],
                    (&prev[pivot] + 1u32).complete(),
                    "pivot {pivot} did not increase by 1 for n={n}, k={k}"
                );
            }

            // indices above pivot unchanged
            for i in (pivot + 1)..k {
                assert_eq!(
                    current[i], prev[i],
                    "index {i} unexpectedly changed for n={n}, k={k}"
                );
            }

            actual.push(current.clone());
        }

        assert_eq!(
            expected.len(),
            actual.len(),
            "length mismatch for n={n}, k={k}"
        );

        for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
            assert_eq!(e, a, "mismatch at index {i} for n={n}, k={k}");
        }
    }

    #[test]
    fn next_multiset_small_cases() {
        check_next_multiset_full(1, 0);
        check_next_multiset_full(1, 1);
        check_next_multiset_full(2, 1);
        check_next_multiset_full(2, 2);
    }

    #[test]
    fn next_multiset_medium_cases() {
        check_next_multiset_full(3, 2);
        check_next_multiset_full(4, 2);
        check_next_multiset_full(4, 3);
    }

    #[test]
    fn next_multiset_larger_cases() {
        check_next_multiset_full(5, 3);
        check_next_multiset_full(6, 3);
        check_next_multiset_full(5, 4);
    }

    #[test]
    fn edge_k_zero() {
        let n = Integer::from(5usize);
        let mut m: Vec<Integer> = Vec::new();

        let adv = Multiset::next_multiset(&mut m, &n);
        assert!(adv.is_none());
        assert!(m.is_empty());
    }

    #[test]
    fn edge_n_zero_k_zero() {
        let n = Integer::from(0);
        let k = 0;

        let total = Multiset::count_multisets(&n, k);
        assert_eq!(total, Integer::from(1usize));

        let mut m = Multiset::unrank(&n, k, &Integer::from(0));
        assert!(m.is_empty());

        let adv = Multiset::next_multiset(&mut m, &n);
        assert!(adv.is_none());
        assert!(m.is_empty());
    }

    #[test]
    fn edge_n_zero_k_positive() {
        let n = Integer::from(0);
        let k = 3;

        let total = Multiset::count_multisets(&n, k);
        assert!(total.is_zero());
    }

    #[test]
    fn edge_n_one() {
        let n = Integer::from(1usize);
        let k = 4;

        let mut m = vec![Integer::from(0); k];

        let adv = Multiset::next_multiset(&mut m, &n);
        assert!(adv.is_none());
        assert_eq!(m, vec![Integer::from(0); k]);
    }

    #[test]
    fn last_multiset_returns_none() {
        let n = Integer::from(5usize);
        let k = 3;

        let total = Multiset::count_multisets(&n, k);
        let last_rank = &total - 1u32;

        let mut last = Multiset::unrank(&n, k, &last_rank.complete());
        let before = last.clone();

        let adv = Multiset::next_multiset(&mut last, &n);
        assert!(adv.is_none());
        assert_eq!(last, before);
    }

    #[test]
    fn batched_sequential_small_n() {
        for n_val in 0usize..8 {
            let n = Integer::from(n_val);

            let ks: Vec<usize> = (0..10).collect();
            let batched = Multiset::count_multisets_batched(&n, &ks);

            for (i, &k) in ks.iter().enumerate() {
                let direct = Multiset::count_multisets(&n, k);
                assert_eq!(batched[i], direct, "Mismatch for n={n_val}, k={k}");
            }
        }
    }
    #[test]
    fn batched_large_gaps() {
        let n = Integer::from(7usize);

        let ks = vec![0, 1, 5, 20, 50];
        let batched = Multiset::count_multisets_batched(&n, &ks);

        for (i, &k) in ks.iter().enumerate() {
            let direct = Multiset::count_multisets(&n, k);
            assert_eq!(batched[i], direct, "Mismatch for n=7, k={k}");
        }
    }
    #[test]
    fn batched_large_integer() {
        let n = Integer::from(100usize);

        let ks = vec![0, 10, 20, 50, 100, 150];
        let batched = Multiset::count_multisets_batched(&n, &ks);

        for (i, &k) in ks.iter().enumerate() {
            let direct = Multiset::count_multisets(&n, k);
            assert_eq!(batched[i], direct, "Mismatch for n=100, k={k}");
        }
    }

    #[test]
    fn batched_dense() {
        let n = Integer::from(6usize);

        let ks: Vec<usize> = (0..25).collect();
        let batched = Multiset::count_multisets_batched(&n, &ks);

        for (i, &k) in ks.iter().enumerate() {
            let direct = Multiset::count_multisets(&n, k);
            assert_eq!(batched[i], direct, "Mismatch at k={k}");
        }
    }
}

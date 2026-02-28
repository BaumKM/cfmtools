use rug::Integer;

use crate::combinatorics::binomial::{Binomial, BinomialStepper};

pub struct Combination;

impl Combination {
    /// Rank a k-combination in colex order.
    ///
    /// `comb` must be strictly increasing.
    #[must_use]
    pub fn rank(comb: &[Integer]) -> Integer {
        let mut rank = Integer::from(0);

        for (i, c) in comb.iter().enumerate() {
            let k = i + 1;
            rank += Binomial::binom(c, k);
        }
        rank
    }

    /// Unrank a k-combination in colex order.
    ///
    /// Input:
    ///   - `n`: number of elements in the base set (elements are `0 .. n-1`)
    ///   - `k`: size of the combination
    ///   - `rank`: rank in `{0, ..., binom(n, k) - 1}`
    ///
    /// Output:
    ///   - strictly increasing sequence `(c_1, ..., c_k)`
    ///     with `0 <= c_i < n`
    #[must_use]
    pub fn unrank(n: &Integer, k: usize, rank: &Integer) -> Vec<Integer> {
        if k == 0 {
            return Vec::new();
        }

        let mut comb = vec![Integer::from(0); k];
        let mut upper = n.clone() - 1;
        let mut rank = rank.clone();

        for i in (1..=k).rev() {
            let mut binom_stepper = BinomialStepper::new(&(i - 1).into(), i);

            // Linear search with incremental binomials
            while binom_stepper.n() < &upper && binom_stepper.peek_next() <= &rank {
                binom_stepper.advance();
            }

            // Commit current contribution
            rank -= binom_stepper.current();

            let x = binom_stepper.n().clone();
            comb[i - 1].clone_from(&x);

            upper = if x.is_zero() { Integer::from(0) } else { x - 1 };
        }

        comb
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    // Roundtrip test: rank(unrank(r)) == r
    #[test]
    fn test_rank_unrank_roundtrip() {
        let k = 4;
        let n = Integer::from(50);
        // Test first 200 ranks
        for rank in 0u128..200 {
            let rank = Integer::from(rank);
            let comb = Combination::unrank(&n, k, &rank);
            let back = Combination::rank(&comb);

            assert_eq!(back, rank, "failed at rank {rank}");
        }
    }

    // Known small values (hand-checked)
    // Colex order for k = 3:
    // rank 0 -> [0,1,2]
    // rank 1 -> [0,1,3]
    // rank 2 -> [0,2,3]
    // rank 3 -> [1,2,3]
    // rank 4 -> [0,1,4]
    #[test]
    fn test_known_small_examples() {
        let k = 3;
        let n = Integer::from(5);

        let cases: Vec<(Integer, Vec<Integer>)> = vec![
            (
                0usize.into(),
                vec![0usize.into(), 1usize.into(), 2usize.into()],
            ),
            (
                1usize.into(),
                vec![0usize.into(), 1usize.into(), 3usize.into()],
            ),
            (
                2usize.into(),
                vec![0usize.into(), 2usize.into(), 3usize.into()],
            ),
            (
                3usize.into(),
                vec![1usize.into(), 2usize.into(), 3usize.into()],
            ),
            (
                4usize.into(),
                vec![0usize.into(), 1usize.into(), 4usize.into()],
            ),
        ];

        for (rank, expected) in cases {
            let comb: Vec<Integer> = Combination::unrank(&n, k, &rank);
            assert_eq!(comb, expected);
        }
    }
}

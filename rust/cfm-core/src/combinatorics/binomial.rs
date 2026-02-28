use rug::{Complete, Integer, integer::IntegerExt64};

pub struct Binomial;

impl Binomial {
    /// Compute binomial coefficient C(n, k)
    #[must_use]
    pub fn binom(n: &Integer, k: usize) -> Integer {
        Integer::from(n).binomial_64(k as u64)
    }
}

/// Incremental binomial coefficient stepper with one-step look-ahead.
///
/// This type represents a moving binomial coefficient C(n, k) together with
/// its successor C(n + 1, k).
///
/// Invariants:
/// - `current == C(n, k)`
/// - `next == C(n + 1, k)`
///
/// The stepper advances monotonically in `n` via [`advance`].
#[derive(Clone, Debug)]
pub struct BinomialStepper {
    n: Integer,
    k: usize,
    current: Integer,
    next: Integer,
}

impl BinomialStepper {
    /// Create an incremental binomial representing C(n, k)
    #[must_use]
    pub fn new(n: &Integer, k: usize) -> Self {
        let current = Binomial::binom(n, k);
        let n_plus_one: Integer = n.clone() + 1;

        let next = if k == 0 {
            Integer::from(1)
        } else if k > n_plus_one {
            // k > n+1  => C(n+1,k) = 0
            Integer::from(0)
        } else if k == n_plus_one {
            // k = n+1 => C(n+1,k) = 1
            Integer::from(1)
        } else {
            (&current * &n_plus_one).complete() / (&n_plus_one - k).complete()
        };

        Self {
            n: n.clone(),
            k,
            current,
            next,
        }
    }

    /// Current C(n, k)
    #[inline]
    #[must_use]
    pub fn current(&self) -> &Integer {
        &self.current
    }

    /// Returns the binomial coefficient for the next value of `n`.
    #[inline]
    #[must_use]
    pub fn peek_next(&self) -> &Integer {
        &self.next
    }

    /// Advance to C(n+1, k)
    ///
    /// Formula:
    ///   C(n+1, k) = C(n, k) * (n+1) / (n+1-k)
    pub fn advance(&mut self) {
        let n_plus_one: Integer = self.n.clone() + 1;

        let n_plus_two: Integer = n_plus_one.clone() + 1;

        let new_next = if self.k == 0 {
            Integer::from(1)
        } else if self.k > n_plus_two {
            Integer::from(0)
        } else if self.k == n_plus_two {
            Integer::from(1)
        } else {
            (&self.next * &n_plus_two).complete() / (&n_plus_two - self.k).complete()
        };

        self.n = n_plus_one;
        self.current = std::mem::replace(&mut self.next, new_next);
    }

    #[inline]
    #[must_use]
    pub fn n(&self) -> &Integer {
        &self.n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binom_trivial_cases() {
        let n = Integer::from(0usize);
        assert_eq!(Binomial::binom(&n, 0), Integer::from(1));
        assert_eq!(Binomial::binom(&n, 1), Integer::from(0));

        let n = Integer::from(10usize);
        assert_eq!(Binomial::binom(&n, 0), Integer::from(1));
        assert_eq!(Binomial::binom(&n, 10), Integer::from(1));
        assert_eq!(Binomial::binom(&n, 11), Integer::from(0));
    }

    #[test]
    fn binom_small_known_values() {
        let n = Integer::from(5usize);
        assert_eq!(Binomial::binom(&n, 1), Integer::from(5usize));
        assert_eq!(Binomial::binom(&n, 2), Integer::from(10usize));
        assert_eq!(Binomial::binom(&n, 3), Integer::from(10usize));
        assert_eq!(Binomial::binom(&n, 4), Integer::from(5usize));
    }

    #[test]
    fn binom_symmetry() {
        let n = Integer::from(20usize);
        for k in 0..=10 {
            let lhs = Binomial::binom(&n, k);
            let rhs = Binomial::binom(&n, 20 - k);
            assert_eq!(lhs, rhs, "symmetry failed for k={k}");
        }
    }

    #[test]
    fn binom_larger_value() {
        let n = Integer::from(50usize);
        let result = Binomial::binom(&n, 6);
        assert_eq!(result, Integer::from(15_890_700usize));
    }

    #[test]
    fn stepper_initialization() {
        let n = Integer::from(5usize);
        let k = 2;

        let stepper = BinomialStepper::new(&n, k);

        assert_eq!(stepper.current(), &Integer::from(10usize));
        assert_eq!(stepper.peek_next(), &Integer::from(15usize));
        assert_eq!(stepper.n(), &n);
    }

    #[test]
    fn stepper_advance_matches_binom() {
        let mut n = Integer::from(7usize);
        let k = 3;

        let mut stepper = BinomialStepper::new(&n, k);

        for _ in 0..10 {
            let expected = Binomial::binom(&n, k);
            assert_eq!(stepper.current(), &expected);
            stepper.advance();
            n += Integer::from(1);
        }
    }

    #[test]
    fn stepper_peek_next_is_correct() {
        let n = Integer::from(8usize);
        let k = 4;

        let stepper = BinomialStepper::new(&n, k);

        let expected_next = Binomial::binom(&(n + Integer::from(1)), k);

        assert_eq!(stepper.peek_next(), &expected_next);
    }

    #[test]
    fn stepper_k_zero_is_always_one() {
        let n = Integer::from(100usize);
        let mut stepper = BinomialStepper::new(&n, 0);

        for _ in 0..20 {
            assert_eq!(stepper.current(), &Integer::from(1));
            assert_eq!(stepper.peek_next(), &Integer::from(1));
            stepper.advance();
        }
    }

    #[test]
    fn stepper_from_below_k_becomes_positive_at_k() {
        let k = 5;
        let start_n = Integer::from(2usize); // n < k

        let mut stepper = BinomialStepper::new(&start_n, k);

        let mut n = start_n;

        // While n < k, C(n, k) must be 0
        while n < k {
            assert_eq!(
                stepper.current(),
                &Integer::from(0),
                "C({n}, {k}) should be 0"
            );

            stepper.advance();
            n += Integer::from(1);
        }

        // At n == k, C(k, k) == 1
        assert_eq!(
            stepper.current(),
            &Integer::from(1),
            "C({n}, {k}) should be 1"
        );

        // One more step: C(k+1, k) == k+1
        stepper.advance();
        n += Integer::from(1);

        assert_eq!(
            stepper.current(),
            &Integer::from(k + 1),
            "C({}, {}) should be {}",
            n,
            k,
            k + 1
        );
    }

    #[test]
    fn binom_large_n_symmetry_small_k() {
        let n = Integer::from(1_000_000usize);
        let k = 5;

        let left = Binomial::binom(&n, k);

        // n - k fits in usize here, so symmetry is valid
        let right = Binomial::binom(&n, 1_000_000 - k);

        assert_eq!(left, right);
    }

    #[test]
    fn binom_large_n_symmetry_medium_k() {
        let n = Integer::from(100_000usize);
        let k = 12_345;

        let left = Binomial::binom(&n, k);
        let right = Binomial::binom(&n, 100_000 - k);

        assert_eq!(left, right);
    }

    #[test]
    fn binom_large_n_symmetry_near_middle() {
        let n = Integer::from(200_000usize);
        let k = 99_999;

        let left = Binomial::binom(&n, k);
        let right = Binomial::binom(&n, 200_000 - k);

        assert_eq!(left, right);
    }
}

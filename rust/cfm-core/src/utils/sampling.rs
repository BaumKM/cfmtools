use rand::{Rng, RngExt};
use rug::{Complete, Integer, rand::RandState};

pub struct CompoundRng<'a, R: Rng> {
    normal: R,
    big: RandState<'a>,
}

impl<'a, R: Rng> CompoundRng<'a, R> {
    pub fn new(mut normal: R) -> Self {
        let seed: u64 = normal.next_u64();

        let mut big = RandState::new();
        big.seed(&Integer::from(seed));

        Self { normal, big }
    }

    #[inline]
    pub fn normal_rng_mut(&mut self) -> &mut R {
        &mut self.normal
    }

    #[inline]
    pub fn bignum_rng_mut(&mut self) -> &mut RandState<'a> {
        &mut self.big
    }

    #[inline]
    pub fn random_below(&mut self, upper: &Integer) -> Integer {
        upper.random_below_ref(&mut self.big).complete()
    }
}

impl<R: Rng> From<R> for CompoundRng<'_, R> {
    fn from(normal: R) -> Self {
        CompoundRng::new(normal)
    }
}

#[derive(Debug, Clone)]
pub struct AliasTable<T> {
    /// Stored objects
    items: Vec<T>,
    /// prob[i] is in the range [0, total]
    probability_table: Vec<Integer>,
    /// alias index
    index_table: Vec<usize>,
    /// sum of all weights
    total: Integer,
}

impl<T> AliasTable<T> {
    /// Build alias table from objects and positive Integer  weights.
    ///
    /// `items.len()` must equal `weights.len()`.
    #[must_use]
    pub fn new(items: Vec<T>, weights: &[Integer]) -> Self {
        assert!(!items.is_empty(), "items must not be empty");
        assert!(
            items.len() == weights.len(),
            "items and weights must have the same length"
        );

        let n = weights.len();

        let total: Integer = weights.iter().sum();
        assert!(total > 0, "sum of weights must be > 0");

        let mut scaled: Vec<Integer> = weights.iter().map(|w| (w * n).complete()).collect();

        //contains elements with weight < total
        let mut underfull: Vec<usize> = Vec::new();
        //contains elements with weight >= total
        let mut overfull: Vec<usize> = Vec::new();

        for (i, s) in scaled.iter().enumerate() {
            if s < &total {
                underfull.push(i);
            } else {
                overfull.push(i);
            }
        }

        let mut probability_table: Vec<Integer> = vec![Integer::from(0); n];
        let mut index_table = vec![0; n];

        while !underfull.is_empty() && !overfull.is_empty() {
            let s = underfull.pop().unwrap();
            let l = overfull.pop().unwrap();

            probability_table[s].clone_from(&scaled[s]);
            index_table[s] = l;

            // Give leftover probability mass to l
            scaled[l] = (&scaled[l] + &scaled[s]).complete() - &total;

            if scaled[l] < total {
                underfull.push(l);
            } else {
                overfull.push(l);
            }
        }

        // Remaining entries have probability = total (always pick themselves)
        for i in underfull.into_iter().chain(overfull) {
            probability_table[i].clone_from(&total);
            index_table[i] = i;
        }

        Self {
            items,
            probability_table,
            index_table,
            total,
        }
    }

    /// Sample an item according to the original weights.
    pub fn sample<R: Rng>(&self, rng: &mut CompoundRng<'_, R>) -> &T {
        let n = self.probability_table.len();

        // select random bucket
        let i = rng.normal_rng_mut().random_range(0..n);

        // Generate Integer in [0, total)
        let r = rng.random_below(&self.total);

        let idx = if r < self.probability_table[i] {
            i
        } else {
            self.index_table[i]
        };

        &self.items[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn single_item_always_selected() {
        let items = vec!["only"];
        let weights = vec![Integer::from(10usize)];

        let table = AliasTable::new(items, &weights);
        let mut rng = StdRng::seed_from_u64(42).into();

        for _ in 0..1000 {
            let v = table.sample(&mut rng);
            assert_eq!(*v, "only");
        }
    }

    #[test]
    fn equal_weights_are_uniform() {
        let items = vec![0, 1, 2, 3];
        let weights = vec![
            Integer::from(1),
            Integer::from(1),
            Integer::from(1),
            Integer::from(1),
        ];

        let table = AliasTable::new(items.clone(), &weights);
        let mut rng = StdRng::seed_from_u64(1234).into();

        let samples = 100_000;
        let mut counts = vec![0usize; items.len()];

        for _ in 0..samples {
            let v = *table.sample(&mut rng);
            counts[v] += 1;
        }

        let expected = f64::from(samples) / items.len() as f64;
        let tolerance = expected * 0.05; // ±5%
        for (i, &count) in counts.iter().enumerate() {
            let diff = (count as f64 - expected).abs();
            assert!(
                diff < tolerance,
                "item {i} count {count} deviates too much from expected {expected}"
            );
        }
    }

    #[test]
    fn weighted_distribution_is_respected() {
        let items = vec![0, 1, 2];
        let weights = vec![
            Integer::from(1usize),
            Integer::from(2usize),
            Integer::from(7usize),
        ]; // ratios 10%, 20%, 70%

        let table = AliasTable::new(items.clone(), &weights);
        let mut rng = StdRng::seed_from_u64(1).into();

        let samples = 200_000;
        let mut counts = vec![0usize; items.len()];

        for _ in 0..samples {
            let v = *table.sample(&mut rng);
            counts[v] += 1;
        }

        let total_weight: Integer = weights.iter().sum();

        for i in 0..items.len() {
            let wi = weights[i].to_f64();
            let tw = total_weight.to_f64();
            let expected_ratio = wi / tw;
            let observed_ratio = counts[i] as f64 / f64::from(samples);

            let diff = (observed_ratio - expected_ratio).abs();
            assert!(
                diff < 0.05, // ±5%
                "item {i} expected {expected_ratio:.3}, observed {observed_ratio:.3}"
            );
        }
    }

    #[test]
    fn zero_weight_items_are_never_selected() {
        let items = vec![0, 1, 2];
        let weights = vec![
            Integer::from(0usize),
            Integer::from(5usize),
            Integer::from(0usize),
        ];

        let table = AliasTable::new(items, &weights);
        let mut rng = StdRng::seed_from_u64(1).into();

        for _ in 0..50_000 {
            let v = *table.sample(&mut rng);
            assert_eq!(v, 1, "only item with positive weight should be selected");
        }
    }
    #[test]
    fn highly_skewed_weights_work() {
        let items = vec![0, 1];
        let weights = vec![Integer::from(1usize), Integer::from(1_000_000usize)];

        let table = AliasTable::new(items, &weights);
        let mut rng = StdRng::seed_from_u64(7).into();

        let samples = 100_000;
        let mut counts = [0usize; 2];

        for _ in 0..samples {
            let v = *table.sample(&mut rng);
            counts[v] += 1;
        }

        // item 1 should dominate heavily
        assert!(counts[1] > (0.99 * f64::from(samples)) as usize);
        assert!(counts[0] < (0.01 * f64::from(samples)) as usize);
    }
}

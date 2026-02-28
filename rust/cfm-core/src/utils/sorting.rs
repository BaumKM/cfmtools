fn bucket_sort(numbers: &mut [usize], max: usize) {
    let mut buckets = vec![0usize; max + 1];

    for &n in numbers.iter() {
        buckets[n] += 1;
    }

    let mut i = 0;
    for (value, &count) in buckets.iter().enumerate() {
        for _ in 0..count {
            numbers[i] = value;
            i += 1;
        }
    }
}

fn bucket_sort_by_key<T, F>(items: &mut Vec<T>, mut key: F)
where
    F: FnMut(&T) -> usize,
{
    if items.is_empty() {
        return;
    }

    let max_key = items.iter().map(&mut key).max().unwrap();
    let mut buckets: Vec<Vec<T>> = (0..=max_key).map(|_| Vec::new()).collect();

    for item in items.drain(..) {
        let k = key(&item);
        buckets[k].push(item);
    }

    for bucket in buckets {
        items.extend(bucket);
    }
}

pub trait BucketSort {
    fn bucket_sort(&mut self);
}

impl BucketSort for Vec<usize> {
    fn bucket_sort(&mut self) {
        let max = self.iter().copied().max().unwrap_or(0);
        bucket_sort(self, max);
    }
}

pub trait BucketSortByKey<T> {
    fn sort_by_key_bucket<F>(&mut self, key: F)
    where
        F: FnMut(&T) -> usize;
}

impl<T> BucketSortByKey<T> for Vec<T> {
    fn sort_by_key_bucket<F>(&mut self, key: F)
    where
        F: FnMut(&T) -> usize,
    {
        bucket_sort_by_key(self, key);
    }
}

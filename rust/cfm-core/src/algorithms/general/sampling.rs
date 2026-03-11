use rand::Rng;
use rug::Complete;
use serde::Serialize;
use std::sync::Arc;

use crate::{
    algorithms::{SampleResult, SampleStatistics, UniformSampler},
    config_spaces::{ConfigSpace, Configuration},
    model::cfm::CFM,
    utils::sampling::CompoundRng,
};

pub struct UniformRankingSampler<Configspace> {
    cfm: Arc<CFM>,
    config_space: Configspace,
}

impl<ConfigSpace> UniformRankingSampler<ConfigSpace> {
    pub fn new(cfm: Arc<CFM>, config_space: ConfigSpace) -> Self {
        Self { cfm, config_space }
    }
}

impl<S> UniformSampler for UniformRankingSampler<S>
where
    S: ConfigSpace,
{
    type Space = S;
    /// Ranking sampler needs no extra preprocessing.
    type SamplerCache = ();

    type Statistics = RankingStatistics;

    fn build_sampler_cache(
        _config_space: &Self::Space,
        _config_cache: &<Self::Space as ConfigSpace>::Cache,
    ) -> Self::SamplerCache {
    }

    fn sample<R: Rng>(
        &self,
        config_cache: &S::Cache,
        _sampler_cache: &Self::SamplerCache,
        rng: &mut CompoundRng<R>,
    ) -> SampleResult<S::Config, RankingStatistics> {
        let count = self.config_space.count(config_cache);
        assert!(!count.is_zero());

        let mut sample_rejections = 0;

        loop {
            let rank = count.random_below_ref(rng.bignum_rng_mut()).complete();
            let config = self.config_space.unrank(config_cache, &rank);

            let counts = config.feature_counts(&self.cfm);
            if self.cfm.satisfies_cross_tree_constraints(counts) {
                return SampleResult {
                    value: config,
                    statistics: RankingStatistics { sample_rejections },
                };
            }
            sample_rejections += 1;
        }
    }

    fn configuration_space(&self) -> &Self::Space {
        &self.config_space
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RankingStatistics {
    pub sample_rejections: usize,
}

impl SampleStatistics for RankingStatistics {
    #[inline]
    fn accumulate(&mut self, other: Self) {
        self.sample_rejections += other.sample_rejections;
    }
}

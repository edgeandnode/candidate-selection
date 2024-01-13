use candidate_selection::criteria::performance::Performance;
use candidate_selection::{self, ArrayVec, Normalized};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash as _, Hasher as _};
use thegraph::types::{Address, DeploymentId};

pub struct Candidate<'p> {
    indexer: Address,
    deployment: DeploymentId,
    fee: Normalized,
    subgraph_versions_behind: u8,
    seconds_behind: u64,
    slashable_usd: u64,
    zero_allocation: bool,
    performance: &'p Performance,
}

const MIN_SCORE_CUTOFF: f64 = 0.25;

pub fn select<'c, Rng, const LIMIT: usize>(
    rng: &mut Rng,
    candidates: &'c [Candidate<'c>],
) -> ArrayVec<&'c Candidate<'c>, LIMIT>
where
    Rng: rand::Rng,
{
    candidate_selection::select(rng, candidates, Normalized::new(MIN_SCORE_CUTOFF).unwrap())
}

impl<'p> candidate_selection::Candidate for Candidate<'p> {
    type Id = u64;

    fn id(&self) -> Self::Id {
        let mut hasher = DefaultHasher::new();
        self.indexer.hash(&mut hasher);
        self.deployment.hash(&mut hasher);
        hasher.finish()
    }

    fn score(&self) -> Normalized {
        [
            score_fee(self.fee),
            score_subgraph_versions_behind(self.subgraph_versions_behind),
            score_seconds_behind(self.seconds_behind),
            score_slashable_usd(self.slashable_usd),
            score_zero_allocation(self.zero_allocation),
            score_latency(self.performance.latency_ms()),
            score_success_rate(self.performance.success_rate()),
        ]
        .into_iter()
        .product()
    }

    fn score_many(candidates: &[&Self]) -> Normalized {
        todo!()
    }
}

/// Score the given `fee`, which is a fraction of some budget. The weight chosen for WPM should be
/// set to target the "optimal" value shown as the vertical line in the following plot.
/// https://www.desmos.com/calculator/wf0tsp1sxh
pub fn score_fee(fee: Normalized) -> Normalized {
    // (5_f64.sqrt() - 1.0) / 2.0
    const S: f64 = 0.6180339887498949;
    let score = (fee.as_f64() + S).recip() - S;
    // Set minimum score, since a very small negative value can result from loss of precision when
    // the fee approaches the budget.
    Normalized::new(score.max(1e-18)).unwrap()
}

fn score_subgraph_versions_behind(subgraph_versions_behind: u8) -> Normalized {
    Normalized::new(MIN_SCORE_CUTOFF.powi(subgraph_versions_behind as i32)).unwrap()
}

fn score_seconds_behind(seconds_behind: u64) -> Normalized {
    todo!()
}

fn score_slashable_usd(slashable_usd: u64) -> Normalized {
    todo!()
}

fn score_zero_allocation(zero_allocation: bool) -> Normalized {
    todo!()
}

/// https://www.desmos.com/calculator/v2vrfktlpl
pub fn score_latency(latency_ms: u32) -> Normalized {
    let sigmoid = |x: u32| 1.0 + std::f64::consts::E.powf(((x as f64) - 400.0) / 300.0);
    Normalized::new(sigmoid(0) / sigmoid(latency_ms)).unwrap()
}

fn score_success_rate(success_rate: Normalized) -> Normalized {
    todo!()
}

#[cfg(test)]
mod test {
    use crate::{score_fee, Normalized};
    use candidate_selection::num::assert_within;
    use proptest::proptest;

    #[test]
    fn fee_limits() {
        assert_within(score_fee(Normalized::ZERO).as_f64(), 1.0, 1e-12);
        assert_within(
            score_fee(Normalized::new(1e-18).unwrap()).as_f64(),
            1.0,
            1e-12,
        );
        assert_within(score_fee(Normalized::ONE).as_f64(), 0.0, 1e-12);
        assert_within(
            score_fee(Normalized::new(1.0 - 1e-18).unwrap()).as_f64(),
            0.0,
            1e-12,
        );
    }

    proptest! {
        #[test]
        fn fee_range(fee in Normalized::arbitrary()) {
            score_fee(fee);
        }
    }
}

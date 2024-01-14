#[cfg(test)]
mod test;

use candidate_selection::criteria::performance::{expected_value_probabilities, Performance};
use candidate_selection::{self, ArrayVec, Normalized};
use std::collections::hash_map::DefaultHasher;
use std::f64::consts::E;
use std::hash::{Hash as _, Hasher as _};
use thegraph::types::{Address, DeploymentId};

#[derive(Debug)]
pub struct Candidate<'p> {
    pub indexer: Address,
    pub deployment: DeploymentId,
    pub fee: Normalized,
    pub subgraph_versions_behind: u8,
    pub seconds_behind: u16,
    pub slashable_usd: u64,
    pub zero_allocation: bool,
    pub performance: &'p Performance,
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

    fn score_many<const LIMIT: usize>(candidates: &[&Self]) -> Normalized {
        let fee = candidates.iter().map(|c| c.fee.as_f64()).sum::<f64>();
        let fee = match Normalized::new(fee) {
            Some(fee) => fee,
            None => return Normalized::ZERO,
        };

        let selections: ArrayVec<&Performance, LIMIT> =
            candidates.iter().map(|c| c.performance).collect();
        let p = expected_value_probabilities::<LIMIT>(&selections);

        let success_rate = Normalized::new(p.iter().map(|p| p.as_f64()).sum()).unwrap();
        let latency = candidates
            .iter()
            .map(|c| c.performance.latency_ms() as f64)
            .zip(&p)
            .map(|(x, p)| x.recip() * p.as_f64())
            .sum::<f64>()
            .recip() as u32;
        let subgraph_versions_behind = candidates
            .iter()
            .map(|c| c.subgraph_versions_behind)
            .zip(&p)
            .map(|(x, p)| x as f64 * p.as_f64())
            .sum::<f64>() as u8;
        let seconds_behind = candidates
            .iter()
            .map(|c| c.seconds_behind)
            .zip(&p)
            .map(|(x, p)| x as f64 * p.as_f64())
            .sum::<f64>() as u16;
        let slashable_usd = candidates
            .iter()
            .map(|c| c.slashable_usd)
            .zip(&p)
            .map(|(x, p)| x as f64 * p.as_f64())
            .sum::<f64>() as u64;
        let p_zero_allocation = candidates
            .iter()
            .map(|c| c.zero_allocation)
            .zip(&p)
            .map(|(x, p)| x as u8 as f64 * p.as_f64())
            .sum::<f64>()
            > 0.5;

        [
            score_fee(fee),
            score_subgraph_versions_behind(subgraph_versions_behind),
            score_seconds_behind(seconds_behind),
            score_slashable_usd(slashable_usd),
            score_zero_allocation(p_zero_allocation),
            score_latency(latency),
            score_success_rate(success_rate),
        ]
        .into_iter()
        .product()
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

/// Avoid serving deployments at versions behind, unless newer versions have poor indexer support.
fn score_subgraph_versions_behind(subgraph_versions_behind: u8) -> Normalized {
    Normalized::new(MIN_SCORE_CUTOFF.powi(subgraph_versions_behind as i32)).unwrap()
}

/// https://www.desmos.com/calculator/wmgkasfvza
fn score_seconds_behind(seconds_behind: u16) -> Normalized {
    let x = seconds_behind as i32;
    let a = 32;
    Normalized::new(1.0 - E.powi(-a * x)).unwrap()
}

/// https://www.desmos.com/calculator/akqaa2gjrf
fn score_slashable_usd(slashable_usd: u64) -> Normalized {
    let x = slashable_usd as f64;
    let a = 2e-4;
    Normalized::new(1.0 - E.powf(-a * x)).unwrap()
}

/// Allocations of zero indicate that an indexer wants lower selection priority.
fn score_zero_allocation(zero_allocation: bool) -> Normalized {
    zero_allocation
        .then(|| Normalized::new(0.8).unwrap())
        .unwrap_or(Normalized::ZERO)
}

/// https://www.desmos.com/calculator/v2vrfktlpl
pub fn score_latency(latency_ms: u32) -> Normalized {
    let sigmoid = |x: u32| 1.0 + std::f64::consts::E.powf(((x as f64) - 400.0) / 300.0);
    Normalized::new(sigmoid(0) / sigmoid(latency_ms)).unwrap()
}

/// https://www.desmos.com/calculator/df2keku3ad
fn score_success_rate(success_rate: Normalized) -> Normalized {
    Normalized::new(success_rate.as_f64().powi(7)).unwrap()
}

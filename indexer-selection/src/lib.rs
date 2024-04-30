use std::collections::hash_map::DefaultHasher;
use std::f64::consts::E;
use std::fmt::Display;
use std::hash::{Hash as _, Hasher as _};

use custom_debug::CustomDebug;
use thegraph_core::types::alloy_primitives::Address;
use thegraph_core::types::DeploymentId;
use url::Url;

use candidate_selection::criteria::performance::expected_value_probabilities;
pub use candidate_selection::criteria::performance::{ExpectedPerformance, Performance};
pub use candidate_selection::{ArrayVec, Normalized};

#[cfg(test)]
mod test;

#[derive(CustomDebug)]
pub struct Candidate {
    pub indexer: Address,
    pub deployment: DeploymentId,
    #[debug(with = Display::fmt)]
    pub url: Url,
    pub perf: ExpectedPerformance,
    pub fee: Normalized,
    pub seconds_behind: u32,
    pub slashable_grt: u64,
    pub subgraph_versions_behind: u8,
    pub zero_allocation: bool,
}

pub fn select<const LIMIT: usize>(candidates: &[Candidate]) -> ArrayVec<&Candidate, LIMIT> {
    candidate_selection::select(candidates)
}

impl candidate_selection::Candidate for Candidate {
    type Id = u64;

    fn id(&self) -> Self::Id {
        let mut hasher = DefaultHasher::new();
        self.indexer.hash(&mut hasher);
        self.deployment.hash(&mut hasher);
        hasher.finish()
    }

    fn fee(&self) -> Normalized {
        self.fee
    }

    fn score(&self) -> Normalized {
        [
            score_success_rate(self.perf.success_rate),
            score_latency(self.perf.latency_ms()),
            score_seconds_behind(self.seconds_behind),
            score_slashable_grt(self.slashable_grt),
            score_subgraph_versions_behind(self.subgraph_versions_behind),
            score_zero_allocation(self.zero_allocation),
        ]
        .into_iter()
        .product()
    }

    fn score_many<const LIMIT: usize>(candidates: &[&Self]) -> Normalized {
        let fee = candidates.iter().map(|c| c.fee.as_f64()).sum::<f64>();
        if Normalized::new(fee).is_none() {
            return Normalized::ZERO;
        }

        let perf: ArrayVec<ExpectedPerformance, LIMIT> =
            candidates.iter().map(|c| c.perf).collect();
        let p = expected_value_probabilities::<LIMIT>(&perf);

        let success_rate = Normalized::new(p.iter().map(|p| p.as_f64()).sum()).unwrap();
        let latency = candidates
            .iter()
            .map(|c| c.perf.latency_ms() as f64)
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
            .sum::<f64>() as u32;
        let slashable_grt = candidates
            .iter()
            .map(|c| c.slashable_grt)
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
            score_success_rate(success_rate),
            score_latency(latency),
            score_seconds_behind(seconds_behind),
            score_slashable_grt(slashable_grt),
            score_subgraph_versions_behind(subgraph_versions_behind),
            score_zero_allocation(p_zero_allocation),
        ]
        .into_iter()
        .product()
    }
}

// When picking curves to use consider the following reference:
// https://en.wikipedia.org/wiki/Logistic_function

/// Avoid serving deployments at versions behind, unless newer versions have poor indexer support.
fn score_subgraph_versions_behind(subgraph_versions_behind: u8) -> Normalized {
    Normalized::new(0.25_f64.powi(subgraph_versions_behind as i32)).unwrap()
}

/// https://www.desmos.com/calculator/gzmp7rbiai
fn score_seconds_behind(seconds_behind: u32) -> Normalized {
    let b: f64 = 1e-6;
    let l: f64 = 1.6;
    let k: f64 = 0.017;
    let x_0: i64 = 30;
    let u = b + (l / (1.0 + E.powf(k * (seconds_behind as i64 - x_0) as f64)));
    Normalized::new(u).unwrap()
}

/// https://www.desmos.com/calculator/iqhjcdnphv
fn score_slashable_grt(slashable_grt: u64) -> Normalized {
    let x = slashable_grt as f64;
    // Currently setting a minimum score of ~0.8 at the minimum stake requirement of 100,000 GRT.
    let a = 1.6e-5;
    Normalized::new(1.0 - E.powf(-a * x)).unwrap()
}

/// Allocations of zero indicate that an indexer wants lower selection priority.
fn score_zero_allocation(zero_allocation: bool) -> Normalized {
    zero_allocation
        .then(|| Normalized::new(0.8).unwrap())
        .unwrap_or(Normalized::ONE)
}

/// https://www.desmos.com/calculator/v2vrfktlpl
pub fn score_latency(latency_ms: u32) -> Normalized {
    let s = |x: u32| 1.0 + E.powf(((x as f64) - 400.0) / 300.0);
    Normalized::new(s(0) / s(latency_ms)).unwrap()
}

/// https://www.desmos.com/calculator/df2keku3ad
fn score_success_rate(success_rate: Normalized) -> Normalized {
    Normalized::new(success_rate.as_f64().powi(7).max(0.01)).unwrap()
}

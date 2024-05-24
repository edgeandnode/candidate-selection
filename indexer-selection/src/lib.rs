mod performance;
#[cfg(test)]
mod test;

pub use candidate_selection::{ArrayVec, Normalized};
use custom_debug::CustomDebug;
pub use performance::*;
use std::{
    collections::hash_map::DefaultHasher,
    f64::consts::E,
    fmt::Display,
    hash::{Hash as _, Hasher as _},
};
use thegraph_core::types::{alloy_primitives::Address, DeploymentId};
use url::Url;

#[derive(CustomDebug)]
pub struct Candidate {
    pub indexer: Address,
    pub deployment: DeploymentId,
    #[debug(with = Display::fmt)]
    pub url: Url,
    pub perf: ExpectedPerformance,
    pub fee: Normalized,
    /// seconds behind chain head
    pub seconds_behind: u32,
    pub slashable_grt: u64,
    /// subgraph versions behind latest deployment
    pub versions_behind: u8,
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
            score_latency(self.perf.latency_ms),
            score_seconds_behind(self.seconds_behind),
            score_slashable_grt(self.slashable_grt),
            score_subgraph_versions_behind(self.versions_behind),
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

        // candidate latencies
        let ls: ArrayVec<u16, LIMIT> = candidates.iter().map(|c| c.perf.latency_ms).collect();
        // probability of candidate responses returning to client, based on `ls`
        let ps = {
            let mut ps: ArrayVec<Normalized, LIMIT> =
                candidates.iter().map(|c| c.perf.success_rate).collect();
            let mut ls = ls.clone();
            let mut sort = permutation::sort_unstable(&mut ls);
            sort.apply_slice_in_place(&mut ls);
            sort.apply_slice_in_place(&mut ps);
            let pf: ArrayVec<f64, LIMIT> = ps
                .iter()
                .map(|p| 1.0 - p.as_f64())
                .scan(1.0, |s, x| {
                    *s *= x;
                    Some(*s)
                })
                .collect();
            let mut ps: ArrayVec<f64, LIMIT> = std::iter::once(&1.0)
                .chain(&pf)
                .take(LIMIT)
                .zip(&ps)
                .map(|(&p, &s)| p * s.as_f64())
                .collect();
            sort.inverse().apply_slice_in_place(&mut ps);
            ps
        };

        let success_rate = Normalized::new(ps.iter().sum()).unwrap_or(Normalized::ONE);
        let latency = candidates
            .iter()
            .map(|c| c.perf.latency_ms as f64)
            .zip(&ps)
            .map(|(x, p)| x.recip() * p)
            .sum::<f64>()
            .recip() as u16;
        let seconds_behind = candidates.iter().map(|c| c.seconds_behind).max().unwrap();
        let slashable_grt = candidates.iter().map(|c| c.slashable_grt).min().unwrap();
        let versions_behind = candidates.iter().map(|c| c.versions_behind).max().unwrap();
        let zero_allocation = candidates.iter().all(|c| c.zero_allocation);

        [
            score_success_rate(success_rate),
            score_latency(latency),
            score_seconds_behind(seconds_behind),
            score_slashable_grt(slashable_grt),
            score_subgraph_versions_behind(versions_behind),
            score_zero_allocation(zero_allocation),
        ]
        .into_iter()
        .product()
    }
}

// When picking curves to use consider the following reference:
// https://en.wikipedia.org/wiki/Logistic_function

/// Avoid serving deployments behind latest version, unless newer versions have poor indexer support.
fn score_subgraph_versions_behind(versions_behind: u8) -> Normalized {
    Normalized::new(0.25_f64.powi(versions_behind as i32)).unwrap()
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
pub fn score_latency(latency_ms: u16) -> Normalized {
    let s = |x: u16| 1.0 + E.powf(((x as f64) - 400.0) / 300.0);
    Normalized::new(s(0) / s(latency_ms)).unwrap()
}

/// https://www.desmos.com/calculator/df2keku3ad
fn score_success_rate(success_rate: Normalized) -> Normalized {
    let min_score = 1e-8;
    Normalized::new(success_rate.as_f64().powi(7).max(min_score)).unwrap()
}

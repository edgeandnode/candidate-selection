use crate::*;
use candidate_selection::{num::assert_within, Candidate as _};
use proptest::{prop_assert, prop_compose, proptest};
use std::ops::RangeInclusive;
use thegraph_core::types::alloy_primitives::{hex, FixedBytes};

#[test]
fn candidate_should_use_url_display_for_debug() {
    let expected_url = "https://example.com/candidate/test/url";
    let candidate = Candidate {
        indexer: Default::default(),
        deployment: FixedBytes::default().into(),
        url: expected_url.parse().expect("valid url"),
        perf: ExpectedPerformance {
            success_rate: Normalized::ZERO,
            latency_ms_p50: 0,
        },
        fee: Normalized::ZERO,
        seconds_behind: 0,
        slashable_grt: 0,
        versions_behind: 0,
        zero_allocation: false,
    };
    assert!(format!("{candidate:?}").contains(expected_url));
}

mod limits {
    use super::*;

    #[test]
    fn success_rate() {
        assert_within(score_success_rate(Normalized::ZERO).as_f64(), 1e-8, 0.001);
    }
}

prop_compose! {
    fn normalized()(n in 0..=10) -> Normalized {
        Normalized::new(n as f64 / 10.0).unwrap()
    }
}
prop_compose! {
    fn candidate()(
        fee in normalized(),
        versions_behind in 0..=3_u8,
        seconds_behind in 0..=7500_u16,
        slashable_grt: u32,
        zero_allocation: bool,
        avg_latency_ms: u16,
        avg_success_rate_percent in 0..=100_u8,
    ) -> Candidate {
        let mut deployment_bytes = [0; 32];
        deployment_bytes[0] = versions_behind;

        let mut performance = Performance::default();
        for _ in 0..avg_success_rate_percent {
            performance.feedback(true, avg_latency_ms);
        }
        for _ in avg_success_rate_percent..100 {
            performance.feedback(false, avg_latency_ms);
        }

        Candidate {
            indexer: Default::default(),
            deployment: deployment_bytes.into(),
            url: "https://example.com".parse().unwrap(),
            perf: ExpectedPerformance { success_rate: performance.success_rate(), latency_ms_p50: performance.latency_percentile(50) },
            fee,
            seconds_behind: seconds_behind as u32,
            slashable_grt: slashable_grt as u64,
            versions_behind,
            zero_allocation,
        }
    }
}
prop_compose! {
    fn candidates(range: RangeInclusive<usize>)(
        mut candidates in proptest::collection::vec(candidate(), range)
    ) -> Vec<Candidate> {
        for (id, candidate) in candidates.iter_mut().enumerate() {
            let mut bytes = [0; 20];
            bytes[0] = id as u8;
            candidate.indexer = bytes.into();
        }
        candidates
    }
}

proptest! {
    #[test]
    fn select(candidates in candidates(1..=5)) {
        println!("scores: {:#?}", candidates.iter().map(|c| (c.indexer, c.score())).collect::<Vec<_>>());
        let selections: ArrayVec<&Candidate, 3> = crate::select(&candidates);
        println!("selections: {:#?}", selections.iter().map(|c| c.indexer).collect::<Vec<_>>());

        let valid_candidate = |c: &Candidate| -> bool {
            c.score() != Normalized::ZERO
        };
        let valid_selections = candidates.iter().filter(|c| valid_candidate(c)).count();

        if valid_selections > 0 {
            prop_assert!(!selections.is_empty(), "some valid candidate selected");
            prop_assert!(selections.len() <= valid_selections, "all candidates selected are valid");
        } else {
            prop_assert!(selections.is_empty(), "no invalid candidate selected");
        }
    }
}

#[test]
fn sensitivity_seconds_behind() {
    let candidates = [
        Candidate {
            indexer: hex!("0000000000000000000000000000000000000000").into(),
            deployment: hex!("0000000000000000000000000000000000000000000000000000000000000000")
                .into(),
            url: "https://example.com".parse().unwrap(),
            perf: ExpectedPerformance {
                success_rate: Normalized::ONE,
                latency_ms_p50: 0,
            },
            fee: Normalized::ZERO,
            seconds_behind: 86400,
            slashable_grt: 1_000_000,
            versions_behind: 0,
            zero_allocation: false,
        },
        Candidate {
            indexer: hex!("0000000000000000000000000000000000000001").into(),
            deployment: hex!("0000000000000000000000000000000000000000000000000000000000000000")
                .into(),
            url: "https://example.com".parse().unwrap(),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.50).unwrap(),
                latency_ms_p50: 1_000,
            },
            fee: Normalized::ONE,
            seconds_behind: 120,
            slashable_grt: 100_000,
            versions_behind: 0,
            zero_allocation: false,
        },
    ];

    println!(
        "score {} {:?} {:?}",
        candidates[0].indexer,
        candidates[0].fee,
        candidates[0].score(),
    );
    println!(
        "score {} {:?} {:?}",
        candidates[1].indexer,
        candidates[1].fee,
        candidates[1].score(),
    );
    assert!(candidates[0].score() <= candidates[1].score());

    let selections: ArrayVec<&Candidate, 3> = crate::select(&candidates);
    assert_eq!(1, selections.len(), "select exatly one candidate");
    assert_eq!(
        Some(candidates[1].indexer),
        selections.first().map(|s| s.indexer),
        "select candidate closer to chain head",
    );
}

#[test]
fn perf_decay() {
    let mut perf = Performance::default();
    let mut candidate = Candidate {
        indexer: hex!("0000000000000000000000000000000000000000").into(),
        deployment: hex!("0000000000000000000000000000000000000000000000000000000000000000").into(),
        url: "https://example.com".parse().unwrap(),
        perf: ExpectedPerformance {
            success_rate: perf.success_rate(),
            latency_ms_p50: perf.latency_percentile(50),
        },
        fee: Normalized::ZERO,
        seconds_behind: 0,
        slashable_grt: 1_000_000,
        versions_behind: 0,
        zero_allocation: false,
    };

    let mut simulate = |seconds, success, latency_ms| {
        let feedback_hz = 20;
        for _ in 0..seconds {
            for _ in 0..feedback_hz {
                perf.feedback(success, latency_ms);
            }
            perf.decay();
        }
        candidate.perf = ExpectedPerformance {
            success_rate: perf.success_rate(),
            latency_ms_p50: perf.latency_percentile(50),
        };
        candidate.score()
    };

    let s0 = simulate(120, true, 200).as_f64();
    let s1 = simulate(2, false, 10).as_f64();
    let s2 = simulate(2, true, 200).as_f64();
    let s3 = simulate(120, true, 200).as_f64();

    println!("{s0:.4}, {s1:.4}, {s2:.4}, {s3:.4}");
    assert_within(s1, s0 * 0.20, 0.05); // fast response
    assert_within(s2, s0 * 0.30, 0.10); // slower recovery
    assert_within(s3, s0 * 1.00, 0.01); // recovery
}

#[test]
fn perf_combine() {
    let candidates = [
        Candidate {
            indexer: hex!("0000000000000000000000000000000000000000").into(),
            deployment: hex!("0000000000000000000000000000000000000000000000000000000000000000")
                .into(),
            url: "https://example.com".parse().unwrap(),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.90).unwrap(),
                latency_ms_p50: 200,
            },
            fee: Normalized::ZERO,
            seconds_behind: 0,
            slashable_grt: 100_000,
            versions_behind: 0,
            zero_allocation: false,
        },
        Candidate {
            indexer: hex!("0000000000000000000000000000000000000001").into(),
            deployment: hex!("0000000000000000000000000000000000000000000000000000000000000000")
                .into(),
            url: "https://example.com".parse().unwrap(),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.90).unwrap(),
                latency_ms_p50: 150,
            },
            fee: Normalized::ZERO,
            seconds_behind: 0,
            slashable_grt: 100_000,
            versions_behind: 0,
            zero_allocation: false,
        },
    ];
    let combined_score = Candidate::score_many::<3>(&[&candidates[0], &candidates[1]]);
    assert!(candidates[0].score() < combined_score);
    assert!(candidates[1].score() < combined_score);
    let selections: ArrayVec<&Candidate, 3> = crate::select(&candidates);
    assert!(selections.len() == 2);
}

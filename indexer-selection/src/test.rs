use std::ops::RangeInclusive;

use alloy_primitives::{hex, Address};
use proptest::{prop_assert, prop_compose, proptest};

use candidate_selection::{num::assert_within, Candidate as _};

use crate::*;

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
    ) -> Candidate<Address, ()> {
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
            id: Default::default(),
            data: (),
            perf: performance.expected_performance(),
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
    ) -> Vec<Candidate<Address, ()>> {
        for (id, candidate) in candidates.iter_mut().enumerate() {
            let mut bytes = [0; 20];
            bytes[0] = id as u8;
            candidate.id = bytes.into();
        }
        candidates
    }
}

proptest! {
    #[test]
    fn select(candidates in candidates(1..=5)) {
        println!("scores: {:#?}", candidates.iter().map(|c| (c.id, c.score())).collect::<Vec<_>>());
        let selections: ArrayVec<&Candidate<Address, ()>, 3> = crate::select(&candidates);
        println!("selections: {:#?}", selections.iter().map(|c| c.id).collect::<Vec<_>>());

        let valid_candidate = |c: &Candidate<Address, ()>| -> bool {
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
            id: hex!("0000000000000000000000000000000000000000").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.99).unwrap(),
                latency_ms: 0,
            },
            fee: Normalized::ZERO,
            seconds_behind: 86400,
            slashable_grt: 1_000_000,
            versions_behind: 0,
            zero_allocation: false,
        },
        Candidate {
            id: hex!("0000000000000000000000000000000000000001").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.5).unwrap(),
                latency_ms: 1000,
            },
            fee: Normalized::ONE,
            seconds_behind: 120,
            slashable_grt: 100_000,
            versions_behind: 0,
            zero_allocation: false,
        },
    ];

    println!("score {} {:?}", candidates[0].id, candidates[0].score(),);
    println!("score {} {:?}", candidates[1].id, candidates[1].score(),);
    assert!(candidates[0].score() <= candidates[1].score());

    let selections: ArrayVec<&Candidate<Address, ()>, 3> = crate::select(&candidates);
    assert_eq!(1, selections.len(), "select exactly one candidate");
    assert_eq!(
        Some(candidates[1].id),
        selections.first().map(|s| s.id),
        "select candidate closer to chain head",
    );
}

#[test]
fn sensitivity_seconds_behind_vs_latency() {
    let candidates = [
        Candidate {
            id: hex!("0000000000000000000000000000000000000000").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.99).unwrap(),
                latency_ms: 0,
            },
            fee: Normalized::ZERO,
            seconds_behind: 35_000_000,
            slashable_grt: 1_600_000,
            versions_behind: 0,
            zero_allocation: false,
        },
        Candidate {
            id: hex!("0000000000000000000000000000000000000001").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.99).unwrap(),
                latency_ms: 10_000,
            },
            fee: Normalized::ZERO,
            seconds_behind: 120,
            slashable_grt: 100_000,
            versions_behind: 0,
            zero_allocation: true,
        },
    ];

    println!("score {} {:?}", candidates[0].id, candidates[0].score(),);
    println!("score {} {:?}", candidates[1].id, candidates[1].score(),);
    assert!(candidates[0].score() <= candidates[1].score());

    let selections: ArrayVec<&Candidate<Address, ()>, 3> = crate::select(&candidates);
    assert_eq!(1, selections.len(), "select exactly one candidate");
    assert_eq!(
        Some(candidates[1].id),
        selections.first().map(|s| s.id),
        "select candidate closer to chain head",
    );
}

#[test]
fn multi_selection_preference() {
    let candidates = [
        Candidate {
            id: hex!("0000000000000000000000000000000000000000").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.99).unwrap(),
                latency_ms: 93,
            },
            fee: Normalized::ZERO,
            seconds_behind: 0,
            slashable_grt: 9445169,
            versions_behind: 0,
            zero_allocation: false,
        },
        Candidate {
            id: hex!("0000000000000000000000000000000000000001").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.99).unwrap(),
                latency_ms: 0,
            },
            fee: Normalized::ZERO,
            seconds_behind: 0,
            slashable_grt: 1330801,
            versions_behind: 0,
            zero_allocation: false,
        },
        Candidate {
            id: hex!("0000000000000000000000000000000000000002").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.99).unwrap(),
                latency_ms: 224,
            },
            fee: Normalized::ZERO,
            seconds_behind: 0,
            slashable_grt: 2675210,
            versions_behind: 0,
            zero_allocation: false,
        },
    ];

    for c in &candidates {
        println!("{} {:?}", c.id, c.score());
    }
    let combined_score = Candidate::score_many::<3>(
        &candidates
            .iter()
            .collect::<ArrayVec<&Candidate<Address, ()>, 3>>(),
    );
    assert!(candidates.iter().all(|c| c.score() < combined_score));

    let selected: ArrayVec<&Candidate<Address, ()>, 3> = crate::select(&candidates);
    println!("{:#?}", selected);
    assert_eq!(3, selected.len(), "all indexers selected");
}

#[test]
fn low_volume_response() {
    let candidates = [
        Candidate {
            id: hex!("0000000000000000000000000000000000000000").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.99).unwrap(),
                latency_ms: 0,
            },
            fee: Normalized::ZERO,
            seconds_behind: 0,
            slashable_grt: 100000,
            versions_behind: 0,
            zero_allocation: false,
        },
        Candidate {
            id: hex!("0000000000000000000000000000000000000001").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.99).unwrap(),
                latency_ms: 0,
            },
            fee: Normalized::ZERO,
            seconds_behind: 0,
            slashable_grt: 100000,
            versions_behind: 0,
            zero_allocation: false,
        },
        Candidate {
            id: hex!("0000000000000000000000000000000000000002").into(),
            data: (),
            perf: ExpectedPerformance {
                success_rate: Normalized::new(0.99).unwrap(),
                latency_ms: 0,
            },
            fee: Normalized::ZERO,
            seconds_behind: 0,
            slashable_grt: 100000,
            versions_behind: 0,
            zero_allocation: true,
        },
    ];

    for c in &candidates {
        println!("{} {:?}", c.id, c.score());
    }
    let combined_score = Candidate::score_many::<3>(
        &candidates
            .iter()
            .collect::<ArrayVec<&Candidate<Address, ()>, 3>>(),
    );
    assert!(candidates.iter().all(|c| c.score() < combined_score));

    let selected: ArrayVec<&Candidate<Address, ()>, 3> = crate::select(&candidates);
    println!("{:#?}", selected);
    assert_eq!(3, selected.len(), "all indexers selected");
}

#[test]
fn perf_decay() {
    let mut perf = Performance::default();
    let mut candidate = Candidate {
        id: hex!("0000000000000000000000000000000000000000"),
        data: (),
        perf: perf.expected_performance(),
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
        candidate.perf = perf.expected_performance();
        candidate.score()
    };

    let s0 = simulate(120, true, 200).as_f64();
    let s1 = simulate(2, false, 10).as_f64();
    let s2 = simulate(8, false, 10).as_f64();
    let s3 = simulate(120, true, 200).as_f64();

    println!("{s0:.4} {s1:.4} {s2:.4} {s3:.4}");
    assert!(s1 < (s0 * 0.8));
    assert!(s2 < (s0 * 0.1));
    assert!(s3 > (s0 * 0.5));
}

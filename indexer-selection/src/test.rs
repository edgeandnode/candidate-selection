use crate::*;
use candidate_selection::{num::assert_within, Candidate as _};
use proptest::{prop_assert, prop_compose, proptest};
use std::ops::RangeInclusive;

mod limits {
    use super::*;

    #[test]
    fn success_rate() {
        assert_within(score_success_rate(Normalized::ZERO).as_f64(), 0.01, 0.001);
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
        subgraph_versions_behind in 0..=3_u8,
        seconds_behind: u16,
        slashable_grt: u32,
        zero_allocation: bool,
        avg_latency_ms: u16,
        avg_success_rate_percent in 0..=100_u8,
    ) -> Candidate {
        let mut deployment_bytes = [0; 32];
        deployment_bytes[0] = subgraph_versions_behind;

        let mut performance = Performance::default();
        for _ in 0..avg_success_rate_percent {
            performance.feedback(true, avg_latency_ms as u32);
        }
        for _ in avg_success_rate_percent..100 {
            performance.feedback(false, avg_latency_ms as u32);
        }

        Candidate {
            indexer: Default::default(),
            deployment: deployment_bytes.into(),
            url: "https://example.com".parse().unwrap(),
            perf: performance.expected_performance(),
            fee,
            seconds_behind: seconds_behind as u32,
            slashable_grt: slashable_grt as u64,
            subgraph_versions_behind,
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

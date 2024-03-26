use crate::*;
use candidate_selection::num::assert_within;
use proptest::{prop_assert, prop_compose, proptest};
use rand::{rngs::SmallRng, SeedableRng};

mod limits {
    use super::*;

    #[test]
    fn fee() {
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

    #[test]
    fn success_rate() {
        assert_within(score_success_rate(Normalized::ZERO).as_f64(), 0.01, 0.001);
    }
}

prop_compose! {
    fn candidates()(
        mut candidates in proptest::collection::vec(candidate(), 1..5)
    ) -> Vec<Candidate> {
        for (id, candidate) in candidates.iter_mut().enumerate() {
            let mut bytes = [0; 20];
            bytes[0] = id as u8;
            candidate.indexer = bytes.into();
        }
        candidates
    }
}
prop_compose! {
    fn candidate()(
        fee in Normalized::arbitrary(),
        subgraph_versions_behind in 0..=3_u8,
        seconds_behind: u16,
        slashable_grt: u64,
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
            indexer: [0; 20].into(),
            deployment: deployment_bytes.into(),
            url: "https://example.com".parse().unwrap(),
            perf: performance.expected_performance(),
            fee,
            seconds_behind: seconds_behind as u32,
            slashable_grt,
            subgraph_versions_behind,
            zero_allocation,
        }
    }
}

proptest! {
    #[test]
    fn select(
        seed: u64,
        candidates in candidates(),
    ) {
        let mut rng = SmallRng::seed_from_u64(seed);
        let selections: ArrayVec<&Candidate, 3> = crate::select(&mut rng, &candidates);
        println!("{:#?}", selections.iter().map(|c| c.indexer).collect::<Vec<_>>());

        let valid_candidate = |c: &Candidate| -> bool {
            c.slashable_grt > 0
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

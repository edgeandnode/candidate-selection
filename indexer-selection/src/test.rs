use crate::*;
use candidate_selection::num::assert_within;
use proptest::{prop_assert, prop_compose, proptest};
use rand::{rngs::SmallRng, SeedableRng};

mod range {
    use super::*;
    proptest! {
        #[test]
        fn fee(fee in Normalized::arbitrary()) {
            score_fee(fee);
        }
        #[test]
        fn subgraph_versions_behind(subgraph_versions_behind: u8) {
            score_subgraph_versions_behind(subgraph_versions_behind);
        }
        #[test]
        fn seconds_behind(seconds_behind: u16) {
            score_seconds_behind(seconds_behind);
        }
        #[test]
        fn slashable_usd(slashable_usd: u64) {
            score_slashable_usd(slashable_usd);
        }
        #[test]
        fn success_rate(success_rate in Normalized::arbitrary()) {
            score_success_rate(success_rate);
        }
        #[test]
        fn zero_allocation(zero_allocation: bool) {
            score_zero_allocation(zero_allocation);
        }
    }
}

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
}

prop_compose! {
    fn candidates()(
        mut candidates in proptest::collection::vec(candidate(), 1..5)
    ) -> Vec<Candidate<'static>> {
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
        slashable_usd: u64,
        zero_allocation: bool,
        avg_latency_ms: u16,
        avg_success_rate_percent in 0..=100_u8,
    ) -> Candidate<'static> {
        let mut deployment_bytes = [0; 32];
        deployment_bytes[0] = subgraph_versions_behind;

        let mut performance = Performance::new();
        for _ in 0..avg_success_rate_percent {
            performance.feedback(true, avg_latency_ms as u32);
        }
        for _ in avg_success_rate_percent..100 {
            performance.feedback(false, avg_latency_ms as u32);
        }

        Candidate {
            indexer: [0; 20].into(),
            deployment: deployment_bytes.into(),
            fee,
            subgraph_versions_behind,
            seconds_behind,
            slashable_usd,
            zero_allocation,
            performance: Box::leak(Box::new(performance)),
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
        let selections: ArrayVec<&Candidate<'_>, 3> = crate::select(&mut rng, &candidates);
        println!("{:#?}", selections.iter().map(|c| c.indexer).collect::<Vec<_>>());
        prop_assert!(!selections.is_empty(), "some valid candidate is selected");
    }
}

use crate::{select, ArrayVec, Candidate, Normalized};
use proptest::{prelude::prop, prop_assert_eq, prop_compose, proptest};

#[derive(Debug)]
struct TestCandidate {
    id: u8,
    fee: Normalized,
    score: Normalized,
}

impl Candidate for TestCandidate {
    type Id = u8;
    fn id(&self) -> Self::Id {
        self.id
    }
    fn fee(&self) -> Normalized {
        self.fee
    }
    fn score(&self) -> Normalized {
        self.score
    }
    fn score_many<const LIMIT: usize>(candidates: &[&Self]) -> Normalized {
        let mut combined_score = 0.0;
        for candidate in candidates {
            combined_score = (combined_score + candidate.score.as_f64()).min(1.0);
        }
        Normalized::new(combined_score).unwrap()
    }
}

prop_compose! {
    fn candidate()(id: u8, fee in Normalized::arbitrary(), score in Normalized::arbitrary()) -> TestCandidate {
        TestCandidate { id, fee, score }
    }
}
proptest! {
    #[test]
    fn acceptable_candidates_selected(
        candidates in prop::collection::vec(candidate(), 1..16),
    ) {
        let exists_acceptable_candidate = candidates.iter().any(|c| c.score > Normalized::ZERO);

        let selections: ArrayVec<&TestCandidate, 1> = select(&candidates);
        prop_assert_eq!(exists_acceptable_candidate, !selections.is_empty());
        prop_assert_eq!(true, selections.iter().all(|s| s.score > Normalized::ZERO));

        let selections: ArrayVec<&TestCandidate, 3> = select(&candidates);
        prop_assert_eq!(true, selections.iter().all(|s| s.score > Normalized::ZERO));
        prop_assert_eq!(exists_acceptable_candidate, !selections.is_empty());
    }
}

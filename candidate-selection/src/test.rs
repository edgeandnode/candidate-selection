use crate::{select, ArrayVec, Candidate, Normalized};
use proptest::{prelude::prop, prop_assert_eq, prop_compose, proptest};
use rand::{rngs::SmallRng, SeedableRng as _};

#[derive(Debug)]
struct TestCandidate {
    id: usize,
    score: Normalized,
}

impl Candidate for TestCandidate {
    type Id = usize;
    fn id(&self) -> Self::Id {
        self.id
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
    fn candidates()(scores in prop::collection::vec(Normalized::arbitrary(), 1..32)) -> Vec<TestCandidate> {
        scores.into_iter().enumerate().map(|(id, score)| TestCandidate { id, score }).collect()
    }
}
proptest! {
    #[test]
    fn acceptable_candidates_selected(
        seed: u64,
        candidates in candidates(),
        min_score_cutoff in Normalized::arbitrary(),
    ) {
        let mut rng = SmallRng::seed_from_u64(seed);
        let exists_acceptable_candidate = candidates.iter().any(|c| c.score > Normalized::ZERO);
        let min_score = candidates
            .iter()
            .filter(|c| c.score > Normalized::ZERO)
            .map(|c| c.score)
            .max()
            .map(|s| s * min_score_cutoff)
            .unwrap_or(Normalized::ZERO);

        let selections: ArrayVec<&TestCandidate, 1> = select(&mut rng, &candidates, min_score_cutoff);
        prop_assert_eq!(exists_acceptable_candidate, !selections.is_empty());
        prop_assert_eq!(true, selections.iter().all(|s| s.score > Normalized::ZERO));
        prop_assert_eq!(true, selections.iter().all(|s| s.score >= min_score));

        let selections: ArrayVec<&TestCandidate, 3> = select(&mut rng, &candidates, min_score_cutoff);
        prop_assert_eq!(true, selections.iter().all(|s| s.score > Normalized::ZERO));
        prop_assert_eq!(exists_acceptable_candidate, !selections.is_empty());
        prop_assert_eq!(true, selections.iter().all(|s| s.score >= min_score));
    }
}

mod normalized;
#[cfg(test)]
mod test;

use std::collections::BTreeMap;

pub use arrayvec::ArrayVec;
use rand::seq::SliceRandom as _;

pub use crate::normalized::Normalized;

pub trait Candidate {
    type Id: Eq + Ord;
    fn id(&self) -> Self::Id;
    fn score(&self) -> Normalized;
    fn score_many(candidates: &[&Self]) -> Normalized;
}

/// Perform a random selection of up to `LIMIT` of the provided candidates. Candidates are picked
/// using a random selection weighted by their individual score. Additional candidates are only
/// added as their inclusion in the selected set increases the combined score of the selected set.
///
/// At least one candidate will be selected, as long as there is at least one candidate with an
/// individual score greater than 0.
///
/// If a candidate's score is below `min_score_cutoff` as a proportion of the max provider's
/// individual score, then the provider will not be selected.
pub fn select<'c, Rng, Candidate, Candidates, const LIMIT: usize>(
    rng: &mut Rng,
    candidates: Candidates,
    min_score_cutoff: Normalized,
) -> ArrayVec<&'c Candidate, LIMIT>
where
    Rng: rand::Rng,
    Candidate: crate::Candidate,
    Candidates: IntoIterator<Item = &'c Candidate>,
{
    assert!(LIMIT > 0);
    // Collect into a map to remove duplicate candidates.
    let candidates: BTreeMap<Candidate::Id, (&'c Candidate, Normalized)> = candidates
        .into_iter()
        .map(|candidate| {
            let score = Candidate::score(candidate);
            (candidate.id(), (candidate, score))
        })
        .filter(|(_, (_, score))| score > &Normalized::ZERO)
        .collect();
    if candidates.is_empty() {
        return ArrayVec::new();
    }
    let max_score = *candidates.values().map(|(_, score)| score).max().unwrap();
    let cutoff_score = max_score * min_score_cutoff;
    // Collect into a vec because `choose_weighted` requires a slice to pick from.
    let mut candidates: Vec<(&'c Candidate, Normalized)> = candidates
        .into_iter()
        .filter(|(_, (_, score))| *score >= cutoff_score)
        .map(|(_, (candidate, score))| (candidate, score))
        .collect();
    // At this point we have reduced the candidates to those with a nonzero score above the cutoff.

    let (first_selection, combined_score) = *candidates
        .choose_weighted(rng, |(_, score)| *score.as_f64())
        .unwrap();
    let mut selections: ArrayVec<&'c Candidate, LIMIT> = Default::default();
    selections.push(first_selection);
    candidates.retain(|(candidate, _)| candidate.id() != first_selection.id());

    // Sample sets of candidates to find combinations that increase the combined score.
    let sample_limit = candidates.len().min(LIMIT * 5);
    for _ in 0..sample_limit {
        if (selections.len() == LIMIT) || candidates.is_empty() {
            break;
        }
        let (picked, _) = *candidates
            .choose_weighted(rng, |(_, score)| *score.as_f64())
            .unwrap();
        selections.push(picked);
        if Candidate::score_many(&selections) > combined_score {
            candidates.retain(|(candidate, _)| candidate.id() != picked.id());
        } else {
            selections.pop();
        }
    }
    selections
}

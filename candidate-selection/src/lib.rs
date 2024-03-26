pub mod criteria;
pub mod num;
#[cfg(test)]
mod test;

pub use crate::num::Normalized;
pub use arrayvec::ArrayVec;
use ordered_float::NotNan;

pub trait Candidate {
    type Id: Eq + Ord;
    fn id(&self) -> Self::Id;
    fn fee(&self) -> Normalized;
    fn score(&self) -> Normalized;
    fn score_many<const LIMIT: usize>(candidates: &[&Self]) -> Normalized;
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
pub fn select<'c, Candidate, const LIMIT: usize>(
    candidates: &'c [Candidate],
) -> ArrayVec<&'c Candidate, LIMIT>
where
    Candidate: crate::Candidate,
{
    assert!(LIMIT > 0);

    let marginal_score = |current_score: Normalized,
                          selected: &ArrayVec<&'c Candidate, LIMIT>,
                          candidate: &'c Candidate| {
        let mut buf = selected.clone();
        buf.push(candidate);
        let potential_score = Candidate::score_many::<LIMIT>(&buf);
        NotNan::new(potential_score.as_f64() - current_score.as_f64()).unwrap()
    };

    let mut selected: ArrayVec<&Candidate, LIMIT> = Default::default();
    while selected.len() < LIMIT {
        let current_score = match selected.len() {
            0 => Normalized::ZERO,
            1 => Candidate::score(selected[0]),
            _ => Candidate::score_many::<LIMIT>(&selected),
        };
        let selection = candidates
            .iter()
            .filter(|c| selected.iter().all(|s| s.id() != c.id()))
            .map(|c| (c, marginal_score(current_score, &selected, c)))
            .max_by_key(|(c, marginal_score)| {
                if c.fee() == Normalized::ZERO {
                    return *marginal_score;
                }
                marginal_score / c.fee().as_f64()
            })
            .filter(|(c, marginal_score)| {
                if current_score == Normalized::ZERO {
                    return true;
                }
                let max_score = 0.5 * *(marginal_score / current_score.as_f64());
                c.fee().as_f64() <= max_score
            });
        match selection {
            Some((selection, _)) => {
                selected.push(selection);
            }
            _ => break,
        };
    }
    selected
}

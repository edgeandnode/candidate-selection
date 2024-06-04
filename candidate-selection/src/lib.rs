pub mod num;
#[cfg(test)]
mod test;

pub use arrayvec::ArrayVec;
use ordered_float::NotNan;

pub use crate::num::Normalized;

pub trait Candidate {
    type Id: Eq + Ord;
    fn id(&self) -> Self::Id;
    fn fee(&self) -> Normalized;
    fn score(&self) -> Normalized;
    fn score_many<const LIMIT: usize>(candidates: &[&Self]) -> Normalized;
}

/// Select up to `LIMIT` of the provided candidates.
///
/// At least one candidate will be selected, as long as there is at least one candidate with an
/// individual score greater than 0.
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
        let potential_score = if selected.is_empty() {
            Candidate::score(candidate)
        } else {
            let mut buf = selected.clone();
            buf.push(candidate);
            Candidate::score_many::<LIMIT>(&buf)
        };
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
            .max_by_key(|(c, marginal_score)| marginal_score / c.fee().as_f64().max(0.01))
            .filter(|(_, marginal_score)| *marginal_score.as_ref() > 0.0);
        match selection {
            Some((selection, _)) => {
                selected.push(selection);
            }
            _ => break,
        };
    }
    selected
}

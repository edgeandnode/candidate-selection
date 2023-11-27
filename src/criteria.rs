pub mod decay;
pub mod performance;

use crate::{Normalized, Weight};

#[derive(Clone, Copy, Debug)]
pub struct Alternative {
    pub score: Normalized,
    pub weight: Weight,
}

/// We use the [weighted product model (WPM)](https://en.wikipedia.org/wiki/Weighted_product_model)
/// to compare candidates across multiple criteria. WPM has the following properties:
/// - One criterion approaching 0 seriously disadvantages a candidate's score.
/// - Raising one criterion's value will always raise the outcome when holding the others constant.
///   This allows us to reason about selection outcomes like "all other things equal, providers with
///   lower latencies are always favored".
pub fn weighted_product_model<Alternatives>(alternatives: Alternatives) -> Normalized
where
    Alternatives: IntoIterator<Item = Alternative>,
{
    alternatives
        .into_iter()
        .map(|Alternative { score, weight }| score.pow(weight))
        .product()
}

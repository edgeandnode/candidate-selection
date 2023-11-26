use ordered_float::NotNan;

use crate::Normalized;

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
    let score = alternatives
        .into_iter()
        .map(|Alternative { score, weight }| score.as_f64().powf(*weight.as_f64()))
        .product();
    Normalized::new(score).unwrap()
}

#[derive(Clone, Copy)]
pub struct Weight(NotNan<f64>);

impl Weight {
    pub fn new(value: f64) -> Option<Self> {
        let value = NotNan::new(value).ok()?;
        if value.is_sign_negative() {
            return None;
        }
        Some(Self(value))
    }

    pub fn as_f64(&self) -> NotNan<f64> {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0.0
    }
}

impl std::fmt::Debug for Weight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

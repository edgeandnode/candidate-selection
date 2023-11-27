use crate::Normalized;

/// Score the given `fee`, which is a fraction of some budget. The weight chosen for WPM should be
/// set to target the "optimal" value shown as the vertical line in the following plot.
/// https://www.desmos.com/calculator/wf0tsp1sxh
pub fn score(fee: Normalized) -> Normalized {
    // (5_f64.sqrt() - 1.0) / 2.0
    const S: f64 = 0.6180339887498949;
    let score = (*fee.as_f64() + S).recip() - S;
    // Set minimum score, since a very small negative value can result from loss of precision when
    // the fee approaches the budget.
    Normalized::new(score.max(1e-18)).unwrap()
}

#[cfg(test)]
mod test {
    use proptest::proptest;

    use crate::{
        test::{assert_within_normalized, normalized},
        Normalized,
    };

    #[test]
    fn fee_limits() {
        assert_within_normalized(super::score(Normalized::ZERO), 1.0, 1e-12);
        assert_within_normalized(super::score(Normalized::new(1e-18).unwrap()), 1.0, 1e-12);
        assert_within_normalized(super::score(Normalized::ONE), 0.0, 1e-12);
        assert_within_normalized(
            super::score(Normalized::new(1.0 - 1e-18).unwrap()),
            0.0,
            1e-12,
        );
    }

    proptest! {
        #[test]
        fn fee_range(fee in normalized()) {
            super::score(fee);
        }
    }
}

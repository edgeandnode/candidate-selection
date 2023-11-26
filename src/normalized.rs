use ordered_float::NotNan;

/// f64 values in the range [0, 1].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Normalized(NotNan<f64>);

impl Normalized {
    pub const ZERO: Self = Self(unsafe { NotNan::new_unchecked(0.0) });
    pub const ONE: Self = Self(unsafe { NotNan::new_unchecked(1.0) });

    pub fn new(value: f64) -> Option<Self> {
        let value = NotNan::new(value).ok()?;
        if value.is_sign_negative() || *value > 1.0 {
            return None;
        }
        Some(Self(value))
    }

    pub fn as_f64(&self) -> NotNan<f64> {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self == &Self::ZERO
    }
}

impl std::ops::Mul for Normalized {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl std::cmp::PartialOrd for Normalized {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl std::cmp::Ord for Normalized {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl std::fmt::Debug for Normalized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

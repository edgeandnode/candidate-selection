use super::decay::{self, DecayBuffer};
use crate::{impl_struct_decay, Normalized};
use arrayvec::ArrayVec;
use ordered_float::NotNan;

/// Tracks success rate & expected latency in milliseconds. For information decay to take effect,
/// `decay` must be called periodically at 1 second intervals.
pub struct Performance {
    latency_success: DecayBuffer<Frame, 7, 4>,
    latency_failure: DecayBuffer<Frame, 7, 4>,
}

#[derive(Default)]
struct Frame {
    total_latency_ms: f64,
    response_count: f64,
}

impl_struct_decay!(Frame {
    total_latency_ms,
    response_count
});

impl Performance {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            latency_success: Default::default(),
            latency_failure: Default::default(),
        }
    }

    pub fn decay(&mut self) {
        self.latency_success.decay();
        self.latency_failure.decay();
    }

    pub fn feedback(&mut self, success: bool, latency_ms: u32) {
        let data_set = if success {
            self.latency_success.current_mut()
        } else {
            self.latency_failure.current_mut()
        };
        data_set.total_latency_ms += latency_ms as f64;
        data_set.response_count += 1.0;
    }

    pub fn success_rate(&self) -> Normalized {
        let successful_responses: f64 = self.latency_success.map(|f| f.response_count).sum();
        let failed_responses: f64 = self.latency_failure.map(|f| f.response_count).sum();
        Normalized::new(successful_responses / (successful_responses + failed_responses).max(1.0))
            .unwrap()
    }

    pub fn latency_ms(&self) -> u32 {
        let p = self.success_rate().as_f64();
        ((*self.latency_success() * p) + (*self.latency_failure() * (1.0 - p))) as u32
    }

    fn latency_success(&self) -> NotNan<f64> {
        let total_latency: f64 = self.latency_success.map(|f| f.total_latency_ms).sum();
        let total_responses: f64 = self.latency_success.map(|f| f.response_count).sum();
        NotNan::new(total_latency / total_responses.max(1.0)).unwrap()
    }

    fn latency_failure(&self) -> NotNan<f64> {
        let total_latency: f64 = self.latency_failure.map(|f| f.total_latency_ms).sum();
        let total_responses: f64 = self.latency_failure.map(|f| f.response_count).sum();
        NotNan::new(total_latency / total_responses.max(1.0)).unwrap()
    }
}

/// Given some combination of selected candidates, return an array of the corresponding
/// probabilities that each candidate's response will be used. This assumes that requests are made
/// in parallel, and that only the first successful response is used.
///
/// For example, with millisecond latencies on successful response `ls = [50, 20, 200]` and success
/// rates `ps = [0.99, 0.5, 0.8]`, the result will be `r = [0.495, 0.5, 0.004]`. To get the expected
/// value for latency, do `ls.iter().zip(r).map(|(l, r)| l.recip() * r).sum().recip()`. The
/// `recip()` calls are only necessary to avoid the expected value tending toward zero when success
/// rates are low (because, for latency, lower is better).
pub fn expected_value_probabilities<const LIMIT: usize>(
    selections: &[&Performance],
) -> ArrayVec<Normalized, LIMIT> {
    let mut ps: ArrayVec<Normalized, LIMIT> = selections.iter().map(|p| p.success_rate()).collect();
    let mut ls: ArrayVec<NotNan<f64>, LIMIT> =
        selections.iter().map(|p| p.latency_success()).collect();

    let mut sort = permutation::sort_unstable_by_key(&mut ls, |r| *r);
    sort.apply_slice_in_place(&mut ps);
    sort.apply_slice_in_place(&mut ls);

    let pf: ArrayVec<f64, LIMIT> = ps
        .iter()
        .map(|p| 1.0 - p.as_f64())
        .scan(1.0, |s, x| {
            *s *= x;
            Some(*s)
        })
        .collect();
    let mut ps: ArrayVec<Normalized, LIMIT> = std::iter::once(&1.0)
        .chain(&pf)
        .take(LIMIT)
        .zip(&ps)
        .map(|(&p, &r)| Normalized::new(p).unwrap() * r)
        .collect();

    sort.inverse().apply_slice_in_place(&mut ps);
    ps
}

#[cfg(test)]
mod test {
    use super::Performance;
    use crate::{num::assert_within, Normalized};
    use arrayvec::ArrayVec;

    #[test]
    fn expected_value_probabilities_example() {
        let mut candidates = [Performance::new(), Performance::new(), Performance::new()];

        for _ in 0..99 {
            candidates[0].feedback(true, 50);
        }
        candidates[0].feedback(false, 50);
        assert_eq!(candidates[0].success_rate().as_f64(), 0.99);
        assert_eq!(candidates[0].latency_ms(), 50);

        candidates[1].feedback(true, 20);
        candidates[1].feedback(false, 20);
        assert_eq!(candidates[1].success_rate().as_f64(), 0.5);
        assert_eq!(candidates[1].latency_ms(), 20);

        for _ in 0..4 {
            candidates[2].feedback(true, 200);
        }
        candidates[2].feedback(false, 200);
        assert_eq!(candidates[2].success_rate().as_f64(), 0.8);
        assert_eq!(candidates[2].latency_ms(), 200);

        let selections: ArrayVec<&Performance, 3> = candidates.iter().collect();
        let result: ArrayVec<Normalized, 3> = super::expected_value_probabilities(&selections);

        assert_within(result[0].as_f64(), 0.495, 1e-4);
        assert_within(result[1].as_f64(), 0.5, 1e-4);
        assert_within(result[2].as_f64(), 0.004, 1e-4);

        let latencies: ArrayVec<u32, 3> = candidates.iter().map(|c| c.latency_ms()).collect();
        let expected_latency = latencies
            .iter()
            .zip(&result)
            .map(|(l, r)| (*l as f64).recip() * r.as_f64())
            .sum::<f64>()
            .recip();
        assert_within(expected_latency, 28.62, 0.02);
    }
}

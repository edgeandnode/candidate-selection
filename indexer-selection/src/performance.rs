use candidate_selection::Normalized;

#[derive(Clone, Default)]
pub struct Performance {
    /// histogram of response latency, in milliseconds
    pub latency_hist: [f32; 29],
    pub failure_count: f64,
}

const LATENCY_BINS: [u16; 29] = [
    32, 64, 96, 128, // + 2^5
    192, 256, 320, 384, // + 2^6
    512, 640, 768, 896, // + 2^7
    1152, 1408, 1664, 1920, // + 2^8
    2432, 2944, 3456, 3968, // + 2^9
    4992, 6016, 7040, 8064, // + 2^10
    10112, 12160, 14208, 16256, // + 2^11
    20352, // + 2^12
];

impl Performance {
    pub fn decay(&mut self) {
        let retain = 0.90;
        self.failure_count *= retain;
        for l in &mut self.latency_hist {
            *l *= retain as f32;
        }
    }

    pub fn feedback(&mut self, success: bool, latency_ms: u16) {
        if !success {
            self.failure_count += 1.0;
        }

        for (count, bin_value) in self
            .latency_hist
            .iter_mut()
            .zip(&LATENCY_BINS)
            .take(LATENCY_BINS.len() - 1)
        {
            if latency_ms <= *bin_value {
                *count += 1.0;
                return;
            }
        }
        *self.latency_hist.last_mut().unwrap() += 1.0;
    }

    pub fn success_rate(&self) -> Normalized {
        let s = self.success_count() + 1.0;
        let f = self.failure_count;
        Normalized::new(s / (s + f)).unwrap()
    }

    pub fn latency_percentile(&self, p: u8) -> u16 {
        debug_assert!((1..=99).contains(&p));
        let target = (self.success_count() + self.failure_count) * (p as f64 / 100.0);
        let mut sum = 0.0;
        for (count, bin_value) in self.latency_hist.iter().zip(&LATENCY_BINS) {
            sum += *count as f64;
            if sum >= target {
                return *bin_value;
            }
        }
        panic!("failed to calculate latency percentile");
    }

    fn success_count(&self) -> f64 {
        let s = self.latency_hist.iter().map(|c| *c as u64).sum::<u64>();
        (s as f64 - self.failure_count).max(0.0)
    }
}

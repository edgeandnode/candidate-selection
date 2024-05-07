use candidate_selection::Normalized;

#[derive(Clone, Debug, Default)]
pub struct Performance {
    fast: ShortTerm,
    slow: LongTerm,
}

const FAST_BIAS: f64 = 0.8;
const FAST_DECAY_HZ: f64 = 0.05;
const SLOW_DECAY_HZ: f64 = 0.001;

#[derive(Clone, Copy, Debug)]
pub struct ExpectedPerformance {
    pub success_rate: Normalized,
    pub latency_ms: u16,
}

impl Performance {
    pub fn expected_performance(&self) -> ExpectedPerformance {
        ExpectedPerformance {
            success_rate: self.success_rate(),
            latency_ms: self.latency_ms(),
        }
    }

    pub fn feedback(&mut self, success: bool, latency_ms: u16) {
        self.fast.feedback(success, latency_ms);
        self.slow.feedback(success, latency_ms);
    }

    pub fn decay(&mut self) {
        self.fast.decay(FAST_DECAY_HZ);
        self.slow.decay(SLOW_DECAY_HZ);
    }

    fn success_rate(&self) -> Normalized {
        let fast = self.fast.success_rate();
        let slow = self.slow.success_rate();
        let success_rate = (fast * FAST_BIAS) + (slow * (1.0 - FAST_BIAS));
        // limit an individual indexer's success rate to 99%
        Normalized::new(success_rate.min(0.99)).unwrap()
    }

    fn latency_ms(&self) -> u16 {
        let fast = self.fast.latency_ms() as f64;
        let slow = self.slow.latency_percentile(99) as f64;
        ((fast * FAST_BIAS) + (slow * (1.0 - FAST_BIAS))) as u16
    }
}

#[derive(Clone, Debug, Default)]
struct ShortTerm {
    total_latency_ms: f64,
    success_count: f64,
    failure_count: f64,
}

impl ShortTerm {
    fn decay(&mut self, rate_hz: f64) {
        debug_assert!((0.0 < rate_hz) && (rate_hz < 1.0));
        let retain = 1.0 - rate_hz;
        self.total_latency_ms *= retain;
        self.success_count *= retain;
        self.failure_count *= retain;
    }

    fn feedback(&mut self, success: bool, latency_ms: u16) {
        self.total_latency_ms += latency_ms as f64;
        if success {
            self.success_count += 1.0;
        } else {
            self.failure_count += 1.0;
        }
    }

    fn success_rate(&self) -> f64 {
        // add 1 to pull success rate upward, and avoid divide by zero
        let s = self.success_count + 1.0;
        let f = self.failure_count;
        s / (s + f)
    }

    fn latency_ms(&self) -> u16 {
        let responses = self.success_count + self.failure_count;
        let avg_latency_ms = self.total_latency_ms / responses.max(1.0);
        avg_latency_ms as u16
    }
}

#[derive(Clone, Debug, Default)]
struct LongTerm {
    latency_hist: [f32; 29],
    failure_count: f64,
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

impl LongTerm {
    fn decay(&mut self, rate_hz: f64) {
        debug_assert!((0.0 < rate_hz) && (rate_hz < 1.0));
        let retain = 1.0 - rate_hz;
        self.failure_count *= retain;
        for count in &mut self.latency_hist {
            *count *= retain as f32;
        }
    }

    fn feedback(&mut self, success: bool, latency_ms: u16) {
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

    fn success_rate(&self) -> f64 {
        // add 1 to pull success rate upward, and avoid divide by zero
        let total = self.latency_hist.iter().map(|c| *c as f64).sum::<f64>() + 1.0;
        let s = total - self.failure_count;
        let f = self.failure_count;
        s / (s + f)
    }

    pub fn latency_percentile(&self, p: u8) -> u16 {
        debug_assert!((1..=99).contains(&p));
        let target = self.latency_hist.iter().map(|c| *c as f64).sum::<f64>() * (p as f64 / 100.0);
        let mut sum = 0.0;
        for (count, bin_value) in self.latency_hist.iter().zip(&LATENCY_BINS) {
            sum += *count as f64;
            if sum >= target {
                return *bin_value;
            }
        }
        panic!("failed to calculate latency percentile");
    }
}

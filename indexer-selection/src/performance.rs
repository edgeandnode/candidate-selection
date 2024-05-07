use candidate_selection::Normalized;

#[derive(Clone, Debug, Default)]
pub struct Performance {
    fast: Frame,
    slow: Frame,
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
        for frame in [&mut self.fast, &mut self.slow] {
            frame.total_latency_ms += latency_ms as f64;
            if success {
                frame.success_count += 1.0;
            } else {
                frame.failure_count += 1.0;
            }
        }
    }

    pub fn decay(&mut self) {
        self.fast.decay(FAST_DECAY_HZ);
        self.slow.decay(SLOW_DECAY_HZ);
    }

    fn success_rate(&self) -> Normalized {
        let fast = self.fast.success_rate().as_f64();
        let slow = self.slow.success_rate().as_f64();
        Normalized::new((fast * FAST_BIAS) + (slow * (1.0 - FAST_BIAS))).unwrap_or(Normalized::ONE)
    }

    fn latency_ms(&self) -> u16 {
        let fast = self.fast.latency_ms() as f64;
        let slow = self.slow.latency_ms() as f64;
        ((fast * FAST_BIAS) + (slow * (1.0 - FAST_BIAS))) as u16
    }
}

#[derive(Clone, Debug, Default)]
struct Frame {
    total_latency_ms: f64,
    success_count: f64,
    failure_count: f64,
}

impl Frame {
    fn decay(&mut self, rate_hz: f64) {
        debug_assert!((0.0 < rate_hz) && (rate_hz < 1.0));
        let retain = 1.0 - rate_hz;
        self.total_latency_ms *= retain;
        self.success_count *= retain;
        self.failure_count *= retain;
    }

    fn success_rate(&self) -> Normalized {
        // add 1 to pull success rate upward
        let s = self.success_count + 1.0;
        let f = self.failure_count;
        let p = s / (s + f);
        // limit an individual indexer's success rate to 99%
        Normalized::new(p.min(0.99)).unwrap()
    }

    fn latency_ms(&self) -> u16 {
        let responses = self.success_count + self.failure_count;
        let avg_latency_ms = self.total_latency_ms / responses.max(1.0);
        avg_latency_ms as u16
    }
}

use candidate_selection::Normalized;

#[derive(Clone, Debug, Default)]
pub struct Performance {
    total_latency_ms: f64,
    success_count: f64,
    failure_count: f64,
}

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
        self.total_latency_ms += latency_ms as f64;
        if success {
            self.success_count += 1.0;
        } else {
            self.failure_count += 1.0;
        }
    }

    pub fn decay(&mut self) {
        let retain = 0.995;
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

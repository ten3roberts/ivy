use std::time::{Duration, Instant};

pub struct Time {
    start_time: Instant,
    pub(crate) elapsed: Duration,
}

impl Time {
    pub fn new(start_time: Instant, elapsed: Duration) -> Self {
        Self {
            start_time,
            elapsed,
        }
    }

    pub fn start_time(&self) -> Instant {
        self.start_time
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

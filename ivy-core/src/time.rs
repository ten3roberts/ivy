//! Provides time related functionality like Clocks and TimeInfo. Also extends Duration for easier
//! construction like 5.secs().
use std::time::{Duration, Instant};

/// Measures high precision time
#[derive(Debug, Clone)]
pub struct Clock {
    start: Instant,
}

impl Clock {
    // Creates and starts a new clock
    pub fn new() -> Self {
        Clock {
            start: Instant::now(),
        }
    }

    // Returns the elapsed time
    pub fn elapsed(&self) -> Duration {
        Instant::now() - self.start
    }

    // Resets the clock and returns the elapsed time
    pub fn reset(&mut self) -> Duration {
        let elapsed = self.elapsed();

        self.start = Instant::now();
        elapsed
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeout {
    timeout: Duration,
    start: Instant,
}

impl Default for Timeout {
    fn default() -> Self {
        Self {
            timeout: Default::default(),
            start: Instant::now(),
        }
    }
}

impl Timeout {
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            start: Instant::now(),
        }
    }

    pub fn empty() -> Self {
        Self {
            timeout: Duration::ZERO,
            start: Instant::now(),
        }
    }

    pub fn set_duration(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = timeout;
        self
    }

    pub fn reset(&mut self) -> &mut Self {
        self.start = Instant::now();
        self
    }

    pub fn is_finished(&self) -> bool {
        self.start.elapsed() >= self.timeout
    }

    pub fn remaining(&self) -> Duration {
        self.timeout - self.start.elapsed()
    }
}

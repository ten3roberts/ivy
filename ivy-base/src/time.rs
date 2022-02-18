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

/// Allows shorter function names to convert duration into intergral types
pub trait FromDuration {
    fn secs(&self) -> f32;
    fn ms(&self) -> u128;
    fn us(&self) -> u128;
    fn ns(&self) -> u128;
}

impl FromDuration for Duration {
    fn secs(&self) -> f32 {
        self.as_secs_f32()
    }

    fn ms(&self) -> u128 {
        self.as_millis()
    }

    fn us(&self) -> u128 {
        self.as_micros()
    }

    fn ns(&self) -> u128 {
        self.as_nanos()
    }
}

/// Trait that allows easier construction of durations
pub trait IntoDuration {
    fn secs(&self) -> Duration;
    fn ms(&self) -> Duration;
    fn us(&self) -> Duration;
    fn ns(&self) -> Duration;
}

impl IntoDuration for u64 {
    fn secs(&self) -> Duration {
        Duration::from_secs(*self)
    }

    fn ms(&self) -> Duration {
        Duration::from_millis(*self)
    }

    fn us(&self) -> Duration {
        Duration::from_micros(*self)
    }

    fn ns(&self) -> Duration {
        Duration::from_nanos(*self)
    }
}

impl IntoDuration for f32 {
    fn secs(&self) -> Duration {
        Duration::from_secs_f32(*self)
    }

    fn ms(&self) -> Duration {
        Duration::from_secs_f64(*self as f64 / 1000.0)
    }

    fn us(&self) -> Duration {
        Duration::from_secs_f64(*self as f64 / 1_000_000.0)
    }

    fn ns(&self) -> Duration {
        Duration::from_secs_f64(*self as f64 / 1_000_000_000.0)
    }
}

/// Times the execution time of a scope and executes the provided function with
/// the results
pub struct TimedScope<F: FnOnce(Duration)> {
    func: Option<F>,
    clock: Clock,
}

impl<F: FnOnce(Duration)> TimedScope<F> {
    pub fn new(func: F) -> Self {
        TimedScope {
            func: Some(func),
            clock: Clock::new(),
        }
    }
}

impl<F: FnOnce(Duration)> Drop for TimedScope<F> {
    fn drop(&mut self) {
        let elapsed = self.clock.elapsed();
        if let Some(f) = self.func.take() {
            f(elapsed)
        }
    }
}

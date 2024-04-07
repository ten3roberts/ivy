//! Provides time related functionality like Clocks and TimeInfo. Also extends Duration for easier
//! construction like 5.secs().
use std::time::{Duration, Instant};

/// Measures high precision time
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
#[deprecated(note = "Use Duration::from methods instead")]
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

impl IntoDuration for i32 {
    fn secs(&self) -> Duration {
        Duration::from_secs(*self as u64)
    }

    fn ms(&self) -> Duration {
        Duration::from_millis(*self as u64)
    }

    fn us(&self) -> Duration {
        Duration::from_micros(*self as u64)
    }

    fn ns(&self) -> Duration {
        Duration::from_nanos(*self as u64)
    }
}

impl IntoDuration for i64 {
    fn secs(&self) -> Duration {
        Duration::from_secs(*self as u64)
    }

    fn ms(&self) -> Duration {
        Duration::from_millis(*self as u64)
    }

    fn us(&self) -> Duration {
        Duration::from_micros(*self as u64)
    }

    fn ns(&self) -> Duration {
        Duration::from_nanos(*self as u64)
    }
}

impl IntoDuration for u32 {
    fn secs(&self) -> Duration {
        Duration::from_secs(*self as u64)
    }

    fn ms(&self) -> Duration {
        Duration::from_millis(*self as u64)
    }

    fn us(&self) -> Duration {
        Duration::from_micros(*self as u64)
    }

    fn ns(&self) -> Duration {
        Duration::from_nanos(*self as u64)
    }
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

impl IntoDuration for f64 {
    fn secs(&self) -> Duration {
        Duration::from_secs_f64(*self)
    }

    fn ms(&self) -> Duration {
        Duration::from_secs_f64(*self / 1000.0)
    }

    fn us(&self) -> Duration {
        Duration::from_secs_f64(*self / 1_000_000.0)
    }

    fn ns(&self) -> Duration {
        Duration::from_secs_f64(*self / 1_000_000_000.0)
    }
}

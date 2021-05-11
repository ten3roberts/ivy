use std::{thread::sleep, time::Duration};

use ivy_core::*;
use log::*;

fn main() {
    // Setup logging
    Logger {
        show_location: false,
        max_level: LevelFilter::Debug,
    }
    .install();

    let mut app = App::builder().push_layer(SandboxLayer::new()).build();

    app.run();
}

struct SandboxLayer {
    frame: usize,
    elapsed: Clock,
    frame_clock: Clock,
    last_status: Clock,
}

impl SandboxLayer {
    fn new() -> Self {
        Self {
            frame: 0,
            frame_clock: Clock::new(),
            elapsed: Clock::new(),
            last_status: Clock::new(),
        }
    }
}

impl Layer for SandboxLayer {
    fn on_update(&mut self) {
        let dt = self.frame_clock.reset();

        if self.last_status.elapsed() > 1.secs() {
            self.last_status.reset();
            info!(
                "Updating SandboxLayer. frame: {}, \telapsed: {:?}, \tdt: {:?}",
                self.frame,
                self.elapsed.elapsed(),
                dt
            );
        }

        self.frame += 1;
        sleep(Duration::from_millis(100));
    }

    fn on_attach(&mut self) {
        info!("Attached sandbox layer");
    }
}

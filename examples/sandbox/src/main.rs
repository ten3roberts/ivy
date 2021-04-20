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
}

impl SandboxLayer {
    fn new() -> Self {
        Self { frame: 0 }
    }
}

impl Layer for SandboxLayer {
    fn on_update(&mut self) {
        info!("Updating frame: {}", self.frame);
        self.frame += 1;
        sleep(Duration::from_millis(100));
    }

    fn on_attach(&mut self) {
        info!("Attached sandbox layer");
    }
}

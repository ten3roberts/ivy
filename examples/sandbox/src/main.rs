use std::{thread::sleep, time::Duration};

use ivy_core::*;

fn main() {
    let mut app = Application::builder()
        .push_layer(SandboxLayer::new())
        .build();

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
        println!("Updating frame: {}", self.frame);
        self.frame += 1;
        sleep(Duration::from_millis(100));
    }

    fn on_attach(&mut self) {
        println!("Attaching sandbox layer");
    }
}

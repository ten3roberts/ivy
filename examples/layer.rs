use ivy_engine::{App, Layer, Logger};
use log::{error, info};

/// Define the game layer
struct GameLayer {}

impl GameLayer {
    fn new() -> Self {
        Self {}
    }
}

impl Layer for GameLayer {
    fn on_update(
        &mut self,
        _: &mut hecs::World,
        _: &mut ivy_engine::Resources,
        _: &mut ivy_base::Events,
        _: std::time::Duration,
    ) -> anyhow::Result<()> {
        info!("Hello, World!");
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    // Install a default logger
    Logger::default().install();

    // Create a simple app
    let result = App::builder()
        .push_layer(|_, _, _| GameLayer::new())
        .build()
        .run();

    // Pretty print results
    match &result {
        Ok(()) => {}
        Err(val) => error!("Error: {}", val),
    }

    result
}

use crate::{App, Clock};

/// Drives the applications main update loop and frequency
pub trait Driver {
    fn enter(&mut self, app: &mut App) -> anyhow::Result<()>;
}

pub struct DefaultDriver {}

impl Driver for DefaultDriver {
    fn enter(&mut self, app: &mut App) -> anyhow::Result<()> {
        app.running = true;

        let mut frame_clock = Clock::new();

        // Update layers
        while app.running {
            let frame_time = frame_clock.reset();
            let world = &mut app.world;
            let asset_cache = &mut app.assets;
            let events = &mut app.events;

            for layer in app.layers.iter_mut() {
                // if let Err(err) = layer.on_update(world, asset_cache, events, frame_time) {
                //     tracing::error!("Error in layer: {:?}", err);
                //     return Err(err);
                // }
            }
        }
        Ok(())
    }
}

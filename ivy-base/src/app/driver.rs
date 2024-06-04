use std::time::Instant;

use crate::App;

/// Drives the applications main update loop and frequency
pub trait Driver {
    fn enter(&mut self, app: &mut App) -> anyhow::Result<()>;
}

pub struct DefaultDriver {}

impl Driver for DefaultDriver {
    fn enter(&mut self, app: &mut App) -> anyhow::Result<()> {
        app.running = true;

        let mut current_time = Instant::now();

        // Update layers
        while app.running {
            let new_time = Instant::now();
            let delta = new_time - current_time;
            current_time = new_time;

            app.tick(delta)?;
        }

        Ok(())
    }
}

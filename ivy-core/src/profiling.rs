pub use ivy_profiling::*;

use crate::{app::TickEvent, Layer};

pub struct ProfilingLayer {
    #[allow(dead_code)]
    #[cfg(feature = "profile")]
    puffin_server: puffin_http::Server,
}

impl ProfilingLayer {
    #[cfg(feature = "profile")]
    pub fn new() -> Self {
        let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
        let puffin_server = puffin_http::Server::new(&server_addr).unwrap();
        tracing::info!("Profiling enabled. Broadcasting on {server_addr}");
        puffin::set_scopes_on(true);

        Self { puffin_server }
    }

    #[cfg(not(feature = "profile"))]
    pub fn new() -> Self {
        Self {}
    }
}

impl Layer for ProfilingLayer {
    fn register(
        &mut self,
        _: &mut flax::World,
        _: &ivy_assets::AssetCache,
        mut events: crate::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        #[cfg(feature = "profile")]
        events.subscribe(|_, _, _, _: &TickEvent| {
            puffin::GlobalProfiler::lock().new_frame();
            Ok(())
        });

        Ok(())
    }
}

impl Default for ProfilingLayer {
    fn default() -> Self {
        Self::new()
    }
}

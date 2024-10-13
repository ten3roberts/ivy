pub use ivy_profiling::*;

use crate::Layer;

pub struct ProfilingLayer {
    #[allow(dead_code)]
    #[cfg(feature = "profile")]
    puffin_server: Option<puffin_http::Server>,
}

impl ProfilingLayer {
    #[cfg(feature = "profile")]
    pub fn new() -> Self {
        let server_addr = format!("127.0.0.1:{}", puffin_http::DEFAULT_PORT);
        let puffin_server = match puffin_http::Server::new(&server_addr) {
            Ok(v) => Some(v),
            Err(_) => {
                tracing::warn!("Failed to bind puffin server");
                None
            }
        };
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
        _events: crate::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        #[cfg(feature = "profile")]
        {
            let mut _events = _events;
            _events.subscribe(|_, _, _, _: &crate::app::TickEvent| {
                puffin::GlobalProfiler::lock().new_frame();
                Ok(())
            });
        }

        Ok(())
    }
}

impl Default for ProfilingLayer {
    fn default() -> Self {
        Self::new()
    }
}

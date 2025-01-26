use flax::Query;
use glam::Mat4;
use ivy_core::{components::main_camera, Layer};
use ivy_wgpu::{components::projection_matrix, events::ResizedEvent, renderer::EnvironmentData};

pub struct CameraSettings {
    pub environment_data: EnvironmentData,
    pub fov: f32,
}

/// Automatically configure a camera based on the window viewport
pub struct ViewportCameraLayer {
    settings: CameraSettings,
}

impl ViewportCameraLayer {
    pub fn new(settings: CameraSettings) -> Self {
        Self { settings }
    }
}

impl Layer for ViewportCameraLayer {
    fn register(
        &mut self,
        _: &mut flax::World,
        _: &ivy_assets::AssetCache,
        mut events: ivy_core::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        let fov = self.settings.fov;
        let environment_data = self.settings.environment_data;
        events.subscribe(move |_, ctx, resized: &ResizedEvent| {
            if let Some((main_camera, environment)) = Query::new((
                projection_matrix().as_mut(),
                ivy_wgpu::components::environment_data().as_mut(),
            ))
            .with(main_camera())
            .borrow(ctx.world)
            .first()
            {
                let aspect =
                    resized.physical_size.width as f32 / resized.physical_size.height as f32;
                *main_camera = Mat4::perspective_rh(fov, aspect, 0.1, 1000.0);
                *environment = environment_data;
            }

            Ok(())
        });

        Ok(())
    }
}

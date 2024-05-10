use flax::World;
use wgpu::{Operations, RenderPassColorAttachment};
use winit::dpi::PhysicalSize;

use crate::{graphics::Surface, Gpu};

// TODO: rendergraph with surface publish node
pub struct Renderer {
    gpu: Gpu,
    surface: Surface,
}

impl Renderer {
    pub fn new(gpu: Gpu, surface: Surface) -> Self {
        Self { gpu, surface }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.surface.resize(&self.gpu, new_size);
    }

    pub fn draw(&mut self, world: &mut World) -> anyhow::Result<()> {
        let output = self.surface.get_current_texture()?;

        let view = output.texture.create_view(&Default::default());

        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: "main_renderpass".into(),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        self.gpu.queue.submit([encoder.finish()]);

        output.present();

        Ok(())
    }
}

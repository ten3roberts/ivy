use std::{mem, ops::Deref};

use anyhow::Context;
use ivy_wgpu::{
    rendergraph::{Dependency, Node, TextureHandle},
    types::PhysicalSize,
    Gpu,
};
use violet::{
    core::components::rect,
    glam::Mat4,
    wgpu::renderer::{MainRenderer, MainRendererConfig, RendererContext},
};
use wgpu::TextureUsages;

use crate::SharedUiInstance;

pub struct UiRenderNode {
    instance: SharedUiInstance,
    renderer: Option<MainRenderer>,
    ctx: RendererContext,
    target: TextureHandle,
}

impl UiRenderNode {
    pub fn new(gpu: &Gpu, ui_instance: SharedUiInstance, target: TextureHandle) -> Self {
        Self {
            instance: ui_instance,
            renderer: None,
            ctx: RendererContext::new(violet::wgpu::Gpu {
                adapter: gpu.adapter.clone(),
                device: gpu.device.clone(),
                queue: gpu.queue.clone(),
            }),
            target,
        }
    }
}

impl Node for UiRenderNode {
    fn draw(&mut self, ctx: ivy_wgpu::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let target = ctx.get_texture(self.target);
        let target_view = target.create_view(&Default::default());

        let instance = &mut *self.instance.deref().borrow_mut();

        if mem::take(&mut instance.needs_update) {
            instance.update();
        }

        let root = instance.frame.world_mut().entity(instance.root)?;

        let size = root
            .get_copy(rect())
            .context("missing size for canvas")?
            .size();

        self.ctx.globals.projview = Mat4::orthographic_lh(0.0, size.x, size.y, 0.0, 0.0, 1000.0);
        self.ctx
            .globals_buffer
            .write(&self.ctx.gpu.queue, 0, &[self.ctx.globals]);

        let renderer = self.renderer.get_or_insert_with(|| {
            let text_system = instance.text_system().clone();
            let layout_changes_rx = instance.layout_changes_rx().clone();
            let root = instance.root();
            let frame = &mut instance.frame;

            MainRenderer::new(
                frame,
                &mut self.ctx,
                root,
                text_system,
                target.format(),
                layout_changes_rx,
                MainRendererConfig { debug_mode: false },
            )
        });

        renderer.resize(
            &self.ctx,
            PhysicalSize {
                width: target.size().width,
                height: target.size().height,
            },
            1.0,
        );

        renderer.update(&mut self.ctx, &mut instance.frame)?;
        renderer.draw(
            &mut self.ctx,
            &mut instance.frame,
            ctx.encoder,
            &target_view,
            false,
        )?;

        Ok(())
    }

    fn on_resource_changed(&mut self, _resource: ivy_wgpu::rendergraph::ResourceHandle) {
        todo!()
    }

    fn read_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![Dependency::texture(
            self.target,
            TextureUsages::RENDER_ATTACHMENT,
        )]
    }

    fn write_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![]
    }
}

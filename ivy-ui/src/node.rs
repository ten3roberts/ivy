use std::{mem, ops::Deref};

use anyhow::Context;
use flax::{filter::ChangeFilter, Component, ComponentMut, FetchExt, Query};
use ivy_wgpu::{
    rendergraph::{Dependency, Node, TextureHandle, UpdateResult},
    types::PhysicalSize,
    Gpu,
};
use violet::{
    core::{assets::Asset, components::rect},
    glam::Mat4,
    wgpu::{
        components::texture_handle,
        renderer::{MainRenderer, MainRendererConfig, RendererContext},
    },
};
use wgpu::{TextureUsages, TextureView};

use crate::{components::texture_dependency, SharedUiInstance};

type TextureDepFetch = (
    Component<TextureHandle>,
    ComponentMut<Option<Asset<TextureView>>>,
);

/// Renders the violet Ui into the rendergraph
pub struct UiRenderNode {
    instance: SharedUiInstance,
    renderer: Option<MainRenderer>,
    ctx: RendererContext,
    target: TextureHandle,
    modified_deps: Query<ChangeFilter<TextureHandle>>,
    texture_deps: Query<TextureDepFetch>,
    update_texture_deps: bool,
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
            modified_deps: Query::new(texture_dependency().modified()),
            texture_deps: Query::new((texture_dependency(), texture_handle().as_mut())),
            update_texture_deps: true,
        }
    }
}

impl Node for UiRenderNode {
    fn update(
        &mut self,
        ctx: ivy_wgpu::rendergraph::NodeUpdateContext,
    ) -> anyhow::Result<UpdateResult> {
        let instance = &mut *self.instance.deref().borrow_mut();
        let new = self
            .modified_deps
            .borrow(instance.frame.world())
            .iter()
            .count()
            > 0;

        if new || self.update_texture_deps {
            self.texture_deps
                .borrow(&instance.frame.world)
                .for_each(|(&handle, view)| {
                    let texture = ctx.get_texture(handle);
                    *view = Some(
                        instance
                            .frame
                            .assets
                            .insert(texture.create_view(&Default::default())),
                    );
                });
        }

        // if new {
        //     return Ok(UpdateResult::RecalculateDepencies);
        // }

        Ok(UpdateResult::Success)
    }

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
        self.update_texture_deps = true;
    }

    fn read_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        let instance = &mut *self.instance.deref().borrow_mut();

        let mut ui_deps = Query::new(
            texture_dependency()
                .copied()
                .map(|v| Dependency::texture(v, TextureUsages::TEXTURE_BINDING)),
        )
        .collect_vec(instance.frame.world());

        ui_deps.push(Dependency::texture(
            self.target,
            TextureUsages::RENDER_ATTACHMENT,
        ));

        ui_deps
    }

    fn write_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![]
    }
}

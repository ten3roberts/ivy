pub mod mesh_renderer;

use std::{arch::global_asm, collections::HashMap};

use bytemuck::Zeroable;
use flax::{FetchExt, Query, World};
use glam::Mat4;
use ivy_assets::{map::AssetMap, Asset, AssetCache, AssetKey};
use ivy_base::{main_camera, world_transform, Bundle};
use wgpu::{
    naga::ShaderStage, util::RenderEncoder, BindGroup, BindGroupLayout, BufferUsages, Operations,
    RenderPass, RenderPassColorAttachment, ShaderStages, TextureFormat,
};
use winit::dpi::PhysicalSize;

use crate::{
    components::{material, mesh, projection_matrix, shader},
    graphics::{
        material::Material, shader::ShaderDesc, BindGroupBuilder, BindGroupLayoutBuilder, Mesh,
        Shader, Surface, TypedBuffer, Vertex, VertexDesc,
    },
    material::MaterialDesc,
    mesh::MeshDesc,
    Gpu,
};

use self::mesh_renderer::MeshRenderer;

// TODO: rendergraph with surface publish node
pub struct Renderer {
    gpu: Gpu,
    surface: Surface,
    mesh_renderer: MeshRenderer,
    globals: Globals,
}

impl Renderer {
    pub fn new(gpu: Gpu, surface: Surface) -> Self {
        Self {
            mesh_renderer: MeshRenderer::new(&gpu, surface.surface_format()),
            globals: Globals::new(&gpu),
            surface,
            gpu,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.surface.resize(&self.gpu, new_size);
    }

    pub fn update(&mut self, world: &World) {
        tracing::debug!("updating renderer");
        if let Some((world_transform, &projection)) =
            Query::new((world_transform(), projection_matrix()))
                .with(main_camera())
                .borrow(world)
                .first()
        {
            let view = world_transform.inverse();

            tracing::debug!("found camera");

            self.globals
                .buffer
                .write(&self.gpu.queue, 0, &[GlobalData { view, projection }]);
        }

        self.mesh_renderer.collect(world);
    }

    pub fn draw(&mut self, assets: &AssetCache) -> anyhow::Result<()> {
        let output = self.surface.get_current_texture()?;

        let view = output.texture.create_view(&Default::default());

        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

            self.mesh_renderer
                .draw(&self.gpu, assets, &self.globals, &mut render_pass);
        }

        self.gpu.queue.submit([encoder.finish()]);

        output.present();

        Ok(())
    }
}

pub struct RenderObject {
    mesh: Asset<MeshDesc>,
    material: Asset<MaterialDesc>,
    shader: Asset<crate::shader::ShaderDesc>,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct ObjectData {
    transform: Mat4,
}

#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct GlobalData {
    view: Mat4,
    projection: Mat4,
}
pub struct Globals {
    bind_group: BindGroup,
    buffer: TypedBuffer<GlobalData>,
    layout: wgpu::BindGroupLayout,
}

impl Globals {
    fn new(gpu: &Gpu) -> Globals {
        let layout = BindGroupLayoutBuilder::new("Globals")
            .bind_uniform_buffer(ShaderStages::VERTEX)
            .build(gpu);

        let buffer = TypedBuffer::new(
            gpu,
            "Globals buffer",
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            &[Default::default()],
        );

        let bind_group = BindGroupBuilder::new("Globals")
            .bind_buffer(&buffer)
            .build(gpu, &layout);

        Self {
            bind_group,
            buffer,
            layout,
        }
    }
}

pub struct RenderObjectBundle {
    pub mesh: Asset<MeshDesc>,
    pub material: Asset<MaterialDesc>,
    pub shader: Asset<crate::shader::ShaderDesc>,
}

impl RenderObjectBundle {
    pub fn new(
        mesh: Asset<MeshDesc>,
        material: Asset<MaterialDesc>,
        shader: Asset<crate::shader::ShaderDesc>,
    ) -> Self {
        Self {
            mesh,
            material,
            shader,
        }
    }
}

impl Bundle for RenderObjectBundle {
    fn mount(self, entity: &mut flax::EntityBuilder) {
        entity
            .set(mesh(), self.mesh)
            .set(material(), self.material)
            .set(shader(), self.shader);
    }
}

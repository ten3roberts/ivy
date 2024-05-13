use bytemuck::Zeroable;
use flax::{FetchExt, Query, World};
use glam::Mat4;
use ivy_assets::Asset;
use ivy_base::{main_camera, world_transform, Bundle};
use wgpu::{
    naga::ShaderStage, util::RenderEncoder, BindGroup, BindGroupLayout, BufferUsages, Operations,
    RenderPass, RenderPassColorAttachment, ShaderStages,
};
use winit::dpi::PhysicalSize;

use crate::{
    components::{material, mesh, projection_matrix, shader},
    graphics::{
        material::Material, shader, BindGroupBuilder, BindGroupLayoutBuilder, Mesh, Shader,
        Surface, TypedBuffer,
    },
    Gpu,
};

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
            mesh_renderer: MeshRenderer::new(&gpu),
            globals: Globals::new(&gpu),
            surface,
            gpu,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.surface.resize(&self.gpu, new_size);
    }

    pub fn update(&mut self, world: &World) {
        tracing::info!("updating renderer");
        if let Some((world_transform, &projection)) =
            Query::new((world_transform(), projection_matrix()))
                .with(main_camera())
                .borrow(world)
                .first()
        {
            let view = world_transform.inverse();

            tracing::info!("found camera");

            self.globals
                .buffer
                .write(&self.gpu.queue, 0, &[GlobalData { view, projection }]);
        }

        self.mesh_renderer.collect(world);
    }

    pub fn draw(&mut self, world: &mut World) -> anyhow::Result<()> {
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
                .draw(&self.gpu, &self.globals, &mut render_pass);
        }

        self.gpu.queue.submit([encoder.finish()]);

        output.present();

        Ok(())
    }
}

pub struct RenderObject {
    mesh: Asset<Mesh>,
    material: Asset<Material>,
    shader: Asset<Shader>,
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct ObjectData {
    transform: Mat4,
}

pub struct MeshRenderer {
    render_objects: Vec<RenderObject>,
    object_data: Vec<ObjectData>,
    object_buffer: TypedBuffer<ObjectData>,

    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
}

impl MeshRenderer {
    fn new(gpu: &Gpu) -> Self {
        let bind_group_layout = BindGroupLayoutBuilder::new("ObjectBuffer")
            .bind_storage_buffer(ShaderStages::VERTEX)
            .build(gpu);

        let object_buffer = TypedBuffer::new(
            gpu,
            "Object buffer",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            &[ObjectData::zeroed(); 8],
        );

        let bind_group = BindGroupBuilder::new("ObjectBuffer")
            .bind_buffer(&object_buffer)
            .build(gpu, &bind_group_layout);

        Self {
            render_objects: Vec::new(),
            object_data: Vec::new(),
            object_buffer,
            bind_group_layout,
            bind_group,
        }
    }

    fn resize_object_buffer(&mut self, gpu: &Gpu, capacity: usize) {
        if self.object_buffer.len() >= capacity {
            return;
        }

        self.object_buffer
            .resize(gpu, capacity.next_power_of_two(), false);

        self.bind_group = BindGroupBuilder::new("ObjectBuffer")
            .bind_buffer(&self.object_buffer)
            .build(gpu, &self.bind_group_layout);
    }

    fn collect(&mut self, world: &World) {
        self.render_objects.clear();
        self.object_data.clear();

        let mut query = Query::new((
            mesh().cloned(),
            material().cloned(),
            shader().cloned(),
            world_transform().copied(),
        ));

        for (mesh, material, shader, transform) in &mut query.borrow(world) {
            self.render_objects.push(RenderObject {
                mesh,
                material,
                shader,
            });
            self.object_data.push(ObjectData { transform });
        }
    }

    fn draw<'a>(&'a mut self, gpu: &Gpu, globals: &'a Globals, render_pass: &mut RenderPass<'a>) {
        self.resize_object_buffer(gpu, self.object_data.len());

        render_pass.set_bind_group(0, &globals.bind_group, &[]);

        self.object_buffer.write(&gpu.queue, 0, &self.object_data);

        tracing::info!("drawing {} objects", self.render_objects.len());
        for (i, render_object) in self.render_objects.iter().enumerate() {
            render_pass.set_pipeline(render_object.shader.pipeline());
            render_object.mesh.bind(render_pass);
            render_pass.set_bind_group(1, &self.bind_group, &[]);
            render_pass.set_bind_group(2, render_object.material.bind_group(), &[]);

            render_pass.draw_indexed(
                0..render_object.mesh.index_count(),
                0,
                (i as u32)..(i as u32 + 1),
            );
        }
    }
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
    pub mesh: Asset<Mesh>,
    pub material: Asset<Material>,
    pub shader: Asset<Shader>,
}

impl RenderObjectBundle {
    pub fn new(mesh: Asset<Mesh>, material: Asset<Material>, shader: Asset<Shader>) -> Self {
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

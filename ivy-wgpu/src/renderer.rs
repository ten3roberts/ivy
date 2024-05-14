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

pub struct MeshRenderer {
    render_objects: Vec<RenderObject>,
    object_data: Vec<ObjectData>,
    object_buffer: TypedBuffer<ObjectData>,

    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,

    pub meshes: AssetMap<MeshDesc, Asset<Mesh>>,
    pub shaders: AssetMap<crate::shader::ShaderDesc, Shader>,
    pub materials: AssetMap<MaterialDesc, Asset<Material>>,

    surface_format: TextureFormat,
}

impl MeshRenderer {
    fn new(gpu: &Gpu, surface_format: TextureFormat) -> Self {
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
            meshes: Default::default(),
            shaders: Default::default(),
            materials: Default::default(),
            surface_format,
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

    fn draw<'a>(
        &'a mut self,
        gpu: &Gpu,
        assets: &AssetCache,
        globals: &'a Globals,
        render_pass: &mut RenderPass<'a>,
    ) {
        self.resize_object_buffer(gpu, self.object_data.len());

        render_pass.set_bind_group(0, &globals.bind_group, &[]);

        self.object_buffer.write(&gpu.queue, 0, &self.object_data);

        for object in &self.render_objects {
            let mesh = {
                self.meshes
                    .entry(&object.mesh)
                    .or_insert_with(|| assets.load(&*object.mesh))
            };
            let material = {
                self.materials
                    .entry(&object.material)
                    .or_insert_with(|| assets.load(&*object.material))
            };
            let shader = {
                let layouts: &[&BindGroupLayout] =
                    &[&globals.layout, &self.bind_group_layout, &material.layout()];
                self.shaders.entry(&object.shader).or_insert_with(|| {
                    Shader::new(
                        gpu,
                        &ShaderDesc {
                            label: object.shader.label(),
                            source: object.shader.source(),
                            format: self.surface_format,
                            vertex_layouts: &[Vertex::layout()],
                            layouts,
                        },
                    )
                })
            };
        }

        tracing::info!("drawing {} objects", self.render_objects.len());
        for (i, object) in self.render_objects.iter().enumerate() {
            let mesh = self.meshes.get(&object.mesh).unwrap();
            let material = self.materials.get(&object.material).unwrap();
            let shader = self.shaders.get(&object.shader).unwrap();

            render_pass.set_pipeline(shader.pipeline());
            mesh.bind(render_pass);
            render_pass.set_bind_group(1, &self.bind_group, &[]);
            render_pass.set_bind_group(2, material.bind_group(), &[]);

            render_pass.draw_indexed(0..mesh.index_count(), 0, (i as u32)..(i as u32 + 1));
        }
    }

    fn get_mesh(&mut self, assets: &AssetCache, mesh: &Asset<MeshDesc>) -> &Asset<Mesh> {
        self.meshes
            .entry(mesh)
            .or_insert_with(|| assets.load(&**mesh))
    }

    fn get_shader(
        &mut self,
        gpu: &Gpu,
        shader: &Asset<crate::shader::ShaderDesc>,
        layouts: &[&BindGroupLayout],
    ) -> &Shader {
        self.shaders.entry(shader).or_insert_with(|| {
            Shader::new(
                gpu,
                &ShaderDesc {
                    label: shader.label(),
                    source: shader.source(),
                    format: self.surface_format,
                    vertex_layouts: &[Vertex::layout()],
                    layouts,
                },
            )
        })
    }

    fn get_material(
        &mut self,
        assets: &AssetCache,
        material: Asset<MaterialDesc>,
    ) -> &Asset<Material> {
        self.materials
            .entry(&material)
            .or_insert_with(|| assets.load(&*material))
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

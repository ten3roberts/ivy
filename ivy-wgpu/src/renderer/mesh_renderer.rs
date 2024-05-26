use std::collections::HashMap;

use bytemuck::Zeroable;
use flax::{FetchExt, Query, World};
use ivy_assets::{map::AssetMap, Asset, AssetCache};
use ivy_base::world_transform;
use wgpu::{BindGroup, BindGroupLayout, BufferUsages, RenderPass, ShaderStages, TextureFormat};

use crate::{
    components::{material, mesh, shader},
    graphics::{
        material::Material, shader::ShaderDesc, BindGroupBuilder, BindGroupLayoutBuilder, Mesh,
        Shader, TypedBuffer, Vertex, VertexDesc,
    },
    material::MaterialDesc,
    mesh::MeshDesc,
    Gpu,
};

use super::{Globals, ObjectData, RenderObject};

pub struct MeshRenderer {
    render_objects: Vec<RenderObject>,
    object_data: Vec<ObjectData>,
    object_buffer: TypedBuffer<ObjectData>,

    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,

    pub meshes: HashMap<MeshDesc, Asset<Mesh>>,
    pub shaders: AssetMap<crate::shader::ShaderDesc, Shader>,
    pub materials: HashMap<MaterialDesc, Asset<Material>>,

    surface_format: TextureFormat,
}

impl MeshRenderer {
    pub fn new(gpu: &Gpu, surface_format: TextureFormat) -> Self {
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

    pub fn collect(&mut self, world: &World) {
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

    pub fn draw<'a>(
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
            self.meshes
                .entry(object.mesh.clone())
                .or_insert_with(|| assets.load(&object.mesh));

            let material = {
                self.materials
                    .entry(object.material.clone())
                    .or_insert_with(|| assets.load(&object.material))
            };

            {
                let layouts: &[&BindGroupLayout] =
                    &[&globals.layout, &self.bind_group_layout, material.layout()];
                self.shaders.entry(&object.shader).or_insert_with(|| {
                    Shader::new(
                        gpu,
                        &ShaderDesc {
                            label: object.shader.label(),
                            source: object.shader.source(),
                            format: self.surface_format,
                            vertex_layouts: &[Vertex::layout()],
                            layouts,
                            depth_format: Some(TextureFormat::Depth24Plus),
                        },
                    )
                })
            };
        }

        tracing::debug!("drawing {} objects", self.render_objects.len());
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
}

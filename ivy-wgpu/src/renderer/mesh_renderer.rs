use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Weak},
};

use bytemuck::Zeroable;
use flax::{
    entity_ids,
    fetch::{Copied, Modified, Source, TransformFetch, Traverse},
    CommandBuffer, Component, Fetch, FetchExt, Query, World,
};
use glam::Mat4;
use ivy_assets::{map::AssetMap, stored::Handle, Asset, AssetCache};
use ivy_base::{world_transform};
use slab::Slab;
use wgpu::{
    BindGroup, BindGroupLayout, BufferUsages, RenderPass,
    ShaderStages, TextureFormat,
};

use crate::{
    components::{material, mesh, mesh_primitive, shader},
    material::MaterialDesc,
    mesh::MeshDesc,
    mesh_buffer::{MeshBuffer, MeshHandle},
    renderer::RendererStore,
    types::{
        material::Material, shader::ShaderDesc, BindGroupBuilder,
        BindGroupLayoutBuilder, Shader, TypedBuffer, Vertex, VertexDesc,
    },
    Gpu,
};

use super::{Globals, ObjectData};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BatchKey {
    pub shader: Asset<crate::shader::ShaderDesc>,
    pub material: MaterialDesc,
    pub mesh: MeshDesc,
}

/// A single rendering batch of similar objects
struct Batch {
    instance_count: u32,
    first_instance: u32,
    instance_capacity: u32,

    mesh: Arc<MeshHandle>,
    material: Asset<Material>,
    shader: Handle<Shader>,
}

impl Batch {
    pub fn new(mesh: Arc<MeshHandle>, material: Asset<Material>, shader: Handle<Shader>) -> Self {
        Self {
            instance_count: 0,
            first_instance: 0,
            mesh,
            material,
            shader,
            instance_capacity: 0,
        }
    }

    pub fn register(&mut self) -> bool {
        self.instance_count += 1;
        self.instance_count > self.instance_capacity
    }

    pub fn draw<'a>(
        &'a self,
        gpu: &Gpu,
        assets: &AssetCache,
        globals: &'a Globals,
        store: &'a RendererStore,
        render_pass: &mut RenderPass<'a>,
    ) {
        render_pass.set_pipeline(store.shaders[&self.shader].pipeline());
        render_pass.set_bind_group(2, self.material.bind_group(), &[]);
        let index_offset = self.mesh.ib().offset() as u32;

        render_pass.draw_indexed(
            index_offset..index_offset + self.mesh.index_count() as u32,
            0,
            self.first_instance..(self.first_instance + self.instance_count),
        );
    }
}

#[derive(Fetch)]
#[fetch(transforms = [ Modified ])]
struct ObjectDataQuery {
    transform: Source<Copied<Component<Mat4>>, Traverse>,
}

impl ObjectDataQuery {
    pub fn new() -> Self {
        Self {
            transform: world_transform().copied().traverse(mesh_primitive),
        }
    }
}

pub struct MeshRenderer {
    /// All the objects registered
    /// |****-|**---|**|
    ///
    /// Sorted by each batch
    object_data: Vec<ObjectData>,
    object_buffer: TypedBuffer<ObjectData>,

    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,

    pub meshes: HashMap<MeshDesc, Weak<MeshHandle>>,
    pub shaders: AssetMap<crate::shader::ShaderDesc, Handle<Shader>>,
    pub materials: HashMap<MaterialDesc, Asset<Material>>,

    surface_format: TextureFormat,

    batches: Slab<Batch>,
    batch_map: BTreeMap<BatchKey, BatchId>,

    object_query: Query<(
        Component<usize>,
        <ObjectDataQuery as TransformFetch<Modified>>::Output,
    )>,

    mesh_buffer: MeshBuffer,
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
            object_data: Vec::new(),
            object_buffer,
            bind_group_layout,
            bind_group,
            meshes: Default::default(),
            shaders: Default::default(),
            materials: Default::default(),
            surface_format,
            batches: Default::default(),
            batch_map: Default::default(),
            object_query: Query::new((object_index(), ObjectDataQuery::new().modified())),
            mesh_buffer: MeshBuffer::new(gpu, "mesh_buffer", 4),
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

    pub fn collect_unbatched(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        globals: &Globals,
        store: &mut RendererStore,
        cmd: &mut CommandBuffer,
    ) {
        let mut query = Query::new((
            entity_ids(),
            mesh().cloned(),
            material().cloned(),
            shader().cloned(),
            ObjectDataQuery::new(),
        ))
        .without(object_index());

        let mut needs_reallocation = false;
        for (id, mesh, material, shader, item) in &mut query.borrow(world) {
            let key = BatchKey {
                mesh,
                material,
                shader,
            };

            let batch_id = *self.batch_map.entry(key).or_insert_with_key(|k| {
                tracing::info!(?k, "creating new batch");
                // TODO: local storage

                let mut load_mesh = |v: &MeshDesc| {
                    let data = v.load_data(assets).unwrap();
                    Arc::new(
                        self.mesh_buffer
                            .insert(gpu, data.vertices(), data.indices()),
                    )
                };

                let mesh = match self.meshes.entry(k.mesh.clone()) {
                    std::collections::hash_map::Entry::Occupied(mut v) => {
                        if let Some(mesh) = v.get().upgrade() {
                            mesh
                        } else {
                            let handle = load_mesh(v.key());
                            v.insert(Arc::downgrade(&handle));
                            handle
                        }
                    }
                    std::collections::hash_map::Entry::Vacant(v) => {
                        let handle = load_mesh(v.key());
                        v.insert(Arc::downgrade(&handle));
                        handle
                    }
                };

                let material = assets.load(&k.material);

                let shader = self.shaders.entry(&k.shader).or_insert_with(|| {
                    store.shaders.insert(Shader::new(
                        gpu,
                        &ShaderDesc {
                            label: k.shader.label(),
                            source: k.shader.source(),
                            format: self.surface_format,
                            vertex_layouts: &[Vertex::layout()],
                            layouts: &[&globals.layout, &self.bind_group_layout, material.layout()],
                            depth_format: Some(TextureFormat::Depth24Plus),
                        },
                    ))
                });

                self.batches
                    .insert(Batch::new(mesh, material, shader.clone()))
            });

            cmd.set(id, object_index(), usize::MAX)
                .set(id, self::batch_id(), batch_id);

            let batch = &mut self.batches[batch_id];

            let index = batch.first_instance + batch.instance_count;
            let object_data = ObjectData {
                transform: item.transform,
            };

            needs_reallocation |= batch.register();
        }

        if needs_reallocation {
            tracing::info!("reallocating object buffer");
            cmd.apply(world).unwrap();
            // TODO: only update positions for new objects
            self.reallocate_object_buffer(world, gpu);
        }
    }

    fn reallocate_object_buffer(&mut self, world: &World, gpu: &Gpu) {
        let mut cursor = 0;
        let mut total_registered = 0;

        for (_, batch) in self.batches.iter_mut() {
            let cap = batch.instance_count.next_power_of_two();
            total_registered += batch.instance_count;
            // Will be restored later
            batch.instance_count = 0;

            batch.first_instance = cursor;
            batch.instance_capacity = cap;
            cursor += cap;
        }

        let mut query = Query::new((batch_id(), object_index().as_mut(), ObjectDataQuery::new()));

        let mut total_found = 0;

        self.object_data.resize(cursor as _, Zeroable::zeroed());
        for (batch_id, object_index, item) in &mut query.borrow(world) {
            let batch = &mut self.batches[*batch_id];
            let index = batch.first_instance + batch.instance_count;
            *object_index = index as usize;
            batch.instance_count += 1;
            total_found += 1;
            assert!(total_found <= total_registered);
            self.object_data[index as usize] = ObjectData {
                transform: item.transform,
            };
        }

        assert_eq!(total_registered, total_found);

        self.resize_object_buffer(gpu, cursor as usize)
    }

    fn update_object_data(&mut self, world: &World, gpu: &Gpu) {
        let mut total = 0;
        for (&object_index, item) in &mut self.object_query.borrow(world) {
            total += 1;
            self.object_data[object_index] = ObjectData {
                transform: item.transform,
            };
        }

        tracing::info!(count = total, "update object");

        self.object_buffer.write(&gpu.queue, 0, &self.object_data);
    }

    pub fn collect(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        store: &mut RendererStore,
        globals: &Globals,
    ) {
        let mut cmd = CommandBuffer::new();
        self.collect_unbatched(world, assets, gpu, globals, store, &mut cmd);
        self.update_object_data(world, gpu);
        // self.render_objects.clear();
        // self.object_data.clear();

        // let mut query = Query::new((
        //     mesh().cloned(),
        //     material().cloned(),
        //     shader().cloned(),
        //     world_transform().copied().traverse(mesh_primitive),
        // ));

        // for (mesh, material, shader, transform) in &mut query.borrow(world) {
        //     self.render_objects.push(RenderObject {
        //         mesh,
        //         material,
        //         shader,
        //     });
        //     self.object_data.push(ObjectData { transform });
        // }
    }

    pub fn draw<'a>(
        &'a mut self,
        assets: &AssetCache,
        gpu: &Gpu,
        globals: &'a Globals,
        store: &'a RendererStore,
        render_pass: &mut RenderPass<'a>,
    ) {
        // self.resize_object_buffer(gpu, self.object_data.len());

        render_pass.set_bind_group(0, &globals.bind_group, &[]);

        self.object_buffer.write(&gpu.queue, 0, &self.object_data);

        tracing::info!("drawing {} batches", self.batch_map.len());

        self.mesh_buffer.bind(render_pass);

        render_pass.set_bind_group(1, &self.bind_group, &[]);
        for batch_id in self.batch_map.values() {
            let batch = &self.batches[*batch_id];
            tracing::info!(instance_count = batch.instance_count, "drawing batch");
            batch.draw(gpu, assets, globals, store, render_pass)
        }

        // for (i, object) in self.render_objects.iter().enumerate() {
        //     let mesh = self.meshes.get(&object.mesh).unwrap();
        //     let material = self.materials.get(&object.material).unwrap();
        //     let shader = self.shaders.get(&object.shader).unwrap();

        //     render_pass.set_pipeline(shader.pipeline());
        //     mesh.bind(render_pass);
        //     render_pass.set_bind_group(1, &self.bind_group, &[]);
        //     render_pass.set_bind_group(2, material.bind_group(), &[]);

        //     render_pass.draw_indexed(0..mesh.index_count(), 0, (i as u32)..(i as u32 + 1));
        // }
    }
}

type BatchId = usize;

flax::component! {
    batch_id: BatchId,
    object_index: usize,
}

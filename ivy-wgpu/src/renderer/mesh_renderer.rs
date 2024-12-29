use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, Weak},
};

use bytemuck::Zeroable;
use flax::{
    components::child_of,
    entity_ids,
    fetch::{entity_refs, Copied, Modified, Source, TransformFetch, Traverse},
    CommandBuffer, Component, Entity, Fetch, FetchExt, Query, World,
};
use glam::Mat4;
use itertools::Itertools;
use ivy_assets::{map::AssetMap, stored::Handle, Asset, AssetCache};
use ivy_core::{
    components::world_transform, profiling::profile_function,
    subscribers::RemovedComponentSubscriber,
};
use ivy_gltf::components::skin;
use ivy_wgpu_types::shader::Culling;
use slab::Slab;
use wgpu::{BindGroup, BindGroupLayout, BufferUsages, DepthBiasState, RenderPass, ShaderStages};

use super::{CameraRenderer, ObjectData, TargetDesc};
use crate::{
    components::mesh,
    material::RenderMaterial,
    material_desc::{MaterialData, PbrMaterialData},
    mesh::{Vertex, VertexDesc},
    mesh_buffer::{MeshBuffer, MeshHandle},
    mesh_desc::MeshDesc,
    renderer::RendererStore,
    shader::ShaderPass,
    shader_library::ShaderLibrary,
    types::{
        shader::ShaderDesc, BindGroupBuilder, BindGroupLayoutBuilder, RenderShader, TypedBuffer,
    },
    Gpu,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub material: MaterialData,
    pub mesh: MeshDesc,
}

/// A single rendering batch of similar objects
struct Batch {
    instance_count: u32,
    first_instance: u32,
    instance_capacity: u32,

    mesh: Arc<MeshHandle>,
    material: Asset<RenderMaterial>,
    shader: Handle<RenderShader>,
}

impl Batch {
    pub fn new(
        mesh: Arc<MeshHandle>,
        material: Asset<RenderMaterial>,
        shader: Handle<RenderShader>,
    ) -> Self {
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
        _: &Gpu,
        _: &AssetCache,
        store: &'a RendererStore,
        render_pass: &mut RenderPass<'a>,
        first_bindgroup: u32,
    ) {
        render_pass.set_pipeline(store.shaders[&self.shader].pipeline());
        if let Some(bind_group) = self.material.bind_group() {
            render_pass.set_bind_group(first_bindgroup, bind_group, &[]);
        }

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
            transform: world_transform().copied().traverse(child_of),
        }
    }
}

impl From<ObjectDataQueryItem<'_>> for ObjectData {
    fn from(value: ObjectDataQueryItem) -> Self {
        Self {
            transform: value.transform,
        }
    }
}

pub type ShaderFactory = Box<dyn FnMut(ShaderDesc) -> ShaderDesc>;

pub struct MeshRenderer {
    id: Entity,
    /// All the objects registered
    /// |****-|**---|**|
    ///
    /// Sorted by each batch
    object_data: Vec<ObjectData>,
    entity_slots: Vec<Option<Entity>>,
    object_buffer: TypedBuffer<ObjectData>,

    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,

    pub meshes: HashMap<MeshDesc, Weak<MeshHandle>>,
    pub shaders: AssetMap<ShaderPass, Handle<RenderShader>>,

    /// Keep track of loaded materials
    // TODO: move higher to deduplicate globally
    pub materials: HashMap<MaterialData, Asset<RenderMaterial>>,

    batches: Slab<Batch>,
    batch_map: HashMap<BatchKey, BatchId>,

    object_query: Query<(
        Component<RendererLocation>,
        <ObjectDataQuery as TransformFetch<Modified>>::Output,
    )>,

    mesh_buffer: MeshBuffer,
    shader_pass: Component<MaterialData>,
    shader_library: Arc<ShaderLibrary>,
    shader_factory: ShaderFactory,
    removed_rx: flume::Receiver<(Entity, RendererLocation)>,
}

impl MeshRenderer {
    pub fn new(
        world: &mut World,
        gpu: &Gpu,
        shader_pass: Component<MaterialData>,
        shader_library: Arc<ShaderLibrary>,
    ) -> Self {
        let id = world.spawn();

        let bind_group_layout = BindGroupLayoutBuilder::new("ObjectBuffer")
            .bind_storage_buffer(ShaderStages::VERTEX)
            .build(gpu);

        let object_buffer = TypedBuffer::new(
            gpu,
            "Object buffer",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            &[ObjectData::zeroed(); 64],
        );

        let bind_group = BindGroupBuilder::new("ObjectBuffer")
            .bind_buffer(&object_buffer)
            .build(gpu, &bind_group_layout);

        let (removed_tx, removed_rx) = flume::unbounded();
        world.subscribe(RemovedComponentSubscriber::new(
            removed_tx,
            renderer_location(id),
        ));

        Self {
            id,
            shader_library,
            object_data: Vec::new(),
            entity_slots: Vec::new(),
            object_buffer,
            bind_group_layout,
            bind_group,
            meshes: Default::default(),
            shaders: Default::default(),
            materials: Default::default(),
            batches: Default::default(),
            batch_map: Default::default(),
            object_query: Query::new((renderer_location(id), ObjectDataQuery::new().modified())),
            mesh_buffer: MeshBuffer::new(gpu, "mesh_buffer", 4),
            shader_pass,
            shader_factory: Box::new(|v| v),
            removed_rx,
        }
    }

    /// Set the shader factory
    pub fn with_shader_factory(
        mut self,
        shader_factory: impl 'static + FnMut(ShaderDesc) -> ShaderDesc,
    ) -> Self {
        self.shader_factory = Box::new(shader_factory);
        self
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

    #[allow(clippy::too_many_arguments)]
    pub fn collect_unbatched(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        layouts: &[&BindGroupLayout],
        store: &mut RendererStore,
        cmd: &mut CommandBuffer,
        target: &TargetDesc,
    ) -> anyhow::Result<()> {
        let mut query = Query::new((
            entity_refs(),
            mesh().cloned(),
            self.shader_pass.cloned(),
            ObjectDataQuery::new(),
        ))
        .without(renderer_location(self.id));

        let mut needs_reallocation = false;
        let mut dirty = false;
        for (entity, mesh, material, object_data) in &mut query.borrow(world) {
            let skin = entity.query(&skin().traverse(child_of)).get().is_some();

            if skin {
                continue;
            }

            dirty = true;

            let id = entity.id();
            let key = BatchKey { mesh, material };

            let mut create_batch = |key: &BatchKey| {
                let mut load_mesh = |v: &MeshDesc| {
                    let mesh_data = v.load_data(assets).unwrap();

                    Arc::new(self.mesh_buffer.insert(
                        gpu,
                        &Vertex::compose_from_mesh(&mesh_data),
                        mesh_data.indices(),
                    ))
                };

                let mesh = match self.meshes.entry(key.mesh.clone()) {
                    Entry::Occupied(mut v) => {
                        if let Some(mesh) = v.get().upgrade() {
                            mesh
                        } else {
                            let handle = load_mesh(v.key());
                            v.insert(Arc::downgrade(&handle));
                            handle
                        }
                    }
                    Entry::Vacant(v) => {
                        let handle = load_mesh(v.key());
                        v.insert(Arc::downgrade(&handle));
                        handle
                    }
                };

                let material: Asset<RenderMaterial> = assets.try_load(&key.material).unwrap_or_else(|e| { tracing::error!(?key.material, "{:?}", e.context("Failed to load material")); assets.load(&MaterialData::PbrMaterial(PbrMaterialData::new())) }) ;

                let shader = material.shader();
                let shader =
                    match self.shaders.entry(shader) {
                        slotmap::secondary::Entry::Occupied(slot) => slot.get().clone(),
                        slotmap::secondary::Entry::Vacant(slot) => {
                            let module = self.shader_library.process(gpu, (&**shader).into())?;

                            let vertex_layouts = &[Vertex::layout()];
                            let bind_group_layouts = layouts
                                .iter()
                                .copied()
                                .chain([&self.bind_group_layout])
                                .chain(material.layout())
                                .collect_vec();

                            let shader_desc = ShaderDesc::new(shader.label(), &module, target)
                                .with_vertex_layouts(vertex_layouts)
                                .with_bind_group_layouts(&bind_group_layouts)
                                .with_culling_mode(Culling {
                                    cull_mode: shader.cull_mode,
                                    front_face: wgpu::FrontFace::Ccw,
                                })
                                .with_depth_bias(DepthBiasState {
                                    constant: -2,
                                    slope_scale: 2.0,
                                    clamp: 0.0,
                                });

                            slot.insert(store.shaders.insert(RenderShader::new(
                                gpu,
                                &(self.shader_factory)(shader_desc),
                            )))
                            .clone()
                        }
                    };

                anyhow::Ok(Batch::new(mesh, material, shader))
            };

            let batch_id = match self.batch_map.entry(key) {
                Entry::Occupied(slot) => *slot.get(),
                Entry::Vacant(slot) => {
                    let batch = create_batch(slot.key());
                    *slot.insert(self.batches.insert(batch?))
                }
            };

            let batch = &mut self.batches[batch_id];

            let index = batch.first_instance + batch.instance_count;
            cmd.set(
                id,
                renderer_location(self.id),
                RendererLocation {
                    batch_id,
                    object_index: index,
                },
            );

            needs_reallocation |= batch.register();

            if !needs_reallocation {
                self.object_data[index as usize] = object_data.into();
                self.entity_slots[index as usize] = Some(id);
            }
        }

        if dirty {
            cmd.apply(world).unwrap();
        }

        if needs_reallocation {
            self.reallocate_object_buffer(world, gpu);
        } else if dirty {
            self.object_buffer.write(&gpu.queue, 0, &self.object_data);
        }

        Ok(())
    }

    fn reallocate_object_buffer(&mut self, world: &World, gpu: &Gpu) {
        profile_function!();

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

        let mut query = Query::new((
            entity_ids(),
            renderer_location(self.id).as_mut(),
            ObjectDataQuery::new(),
        ));

        let mut total_found = 0;

        self.object_data.resize(cursor as _, Zeroable::zeroed());
        self.entity_slots.resize(cursor as _, None);
        for (id, loc, item) in &mut query.borrow(world) {
            let batch = &mut self.batches[loc.batch_id];
            let index = batch.first_instance + batch.instance_count;
            loc.object_index = index;
            batch.instance_count += 1;
            total_found += 1;
            assert!(total_found <= total_registered);
            self.object_data[index as usize] = item.into();
            self.entity_slots[index as usize] = Some(id);
        }

        assert!(
            total_registered >= total_found,
            "{total_registered} > {total_found}"
        );

        self.resize_object_buffer(gpu, cursor as usize)
    }

    pub fn handle_removed(&mut self, world: &World) {
        for (id, loc) in self.removed_rx.try_iter() {
            let batch = &mut self.batches[loc.batch_id];
            let batch_end = batch.first_instance + batch.instance_count;

            if loc.object_index == batch_end - 1 {
                assert!(self.entity_slots[loc.object_index as usize] == Some(id));
                self.entity_slots[loc.object_index as usize] = None;
                batch.instance_count -= 1;
            } else {
                let slot = loc.object_index as usize;
                self.object_data.swap(slot, batch_end as usize - 1);
                self.entity_slots.swap(slot, batch_end as usize - 1);

                let swapped_entity =
                    self.entity_slots[slot].expect("Entity must be present in slot");

                let _ = world.update(swapped_entity, renderer_location(self.id), |v| {
                    v.object_index = loc.object_index;
                });

                batch.instance_count -= 1;
            }
        }
    }

    fn update_object_data(&mut self, world: &World, gpu: &Gpu) {
        for (loc, item) in &mut self.object_query.borrow(world) {
            assert_ne!(loc.object_index, u32::MAX);
            self.object_data[loc.object_index as usize] = ObjectData {
                transform: item.transform,
            };
        }

        self.object_buffer.write(&gpu.queue, 0, &self.object_data);
    }
}

impl CameraRenderer for MeshRenderer {
    fn update(&mut self, ctx: &mut super::UpdateContext) -> anyhow::Result<()> {
        profile_function!();
        let mut cmd = CommandBuffer::new();
        self.handle_removed(ctx.world);
        self.collect_unbatched(
            ctx.world,
            ctx.assets,
            ctx.gpu,
            ctx.layouts,
            ctx.store,
            &mut cmd,
            &ctx.target_desc,
        )?;

        self.update_object_data(ctx.world, ctx.gpu);

        Ok(())
    }

    fn draw<'s>(
        &'s mut self,
        ctx: &'s super::RenderContext<'s>,
        render_pass: &mut RenderPass<'s>,
    ) -> anyhow::Result<()> {
        profile_function!();

        for (i, bind_group) in ctx.bind_groups.iter().enumerate() {
            render_pass.set_bind_group(i as _, bind_group, &[]);
        }

        self.object_buffer
            .write(&ctx.gpu.queue, 0, &self.object_data);

        tracing::trace!("drawing {} batches", self.batch_map.len());

        self.mesh_buffer.bind(render_pass);

        render_pass.set_bind_group(ctx.bind_groups.len() as _, &self.bind_group, &[]);
        for batch_id in self.batch_map.values() {
            let batch = &self.batches[*batch_id];
            tracing::trace!(instance_count = batch.instance_count, "drawing batch");
            batch.draw(
                ctx.gpu,
                ctx.assets,
                ctx.store,
                render_pass,
                ctx.bind_groups.len() as u32 + 1,
            )
        }

        Ok(())
    }
}

type BatchId = usize;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
struct RendererLocation {
    batch_id: BatchId,
    object_index: u32,
}

flax::component! {
    renderer_location(id): RendererLocation,
}

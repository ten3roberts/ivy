use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Weak},
};

use bytemuck::Zeroable;
use flax::{
    entity_ids,
    fetch::{Copied, Modified, Source, TransformFetch, Traverse},
    CommandBuffer, Component, Entity, Fetch, FetchExt, Query, World,
};
use glam::{Mat4, Vec4Swizzles};
use itertools::Itertools;
use ivy_assets::{map::AssetMap, stored::Handle, Asset, AssetCache};
use ivy_core::world_transform;
use ivy_gltf::{
    animation::{player::Animator, skin::Skin},
    components::{animator, skin},
};
use ivy_wgpu_types::shader::TargetDesc;
use slab::Slab;
use wgpu::{BindGroup, BindGroupLayout, BufferUsages, RenderPass, ShaderStages, TextureFormat};

use crate::{
    components::{forward_pass, material, mesh, mesh_primitive},
    material::Material,
    material_desc::{MaterialData, MaterialDesc},
    mesh::{SkinnedVertex, VertexDesc},
    mesh_buffer::{MeshBuffer, MeshHandle},
    mesh_desc::MeshDesc,
    renderer::RendererStore,
    shader::ShaderPassDesc,
    types::{shader::ShaderDesc, BindGroupBuilder, BindGroupLayoutBuilder, Shader, TypedBuffer},
    Gpu,
};

use super::{CameraRenderer, CameraShaderData, ObjectData};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub skin: Asset<Skin>,
    pub shader: Asset<crate::shader::ShaderPassDesc>,
    pub material: MaterialDesc,
    pub mesh: MeshDesc,
}

/// A single rendering batch of similar objects
struct Batch {
    instance_count: u32,
    first_instance: u32,
    instance_capacity: u32,

    skin: Asset<Skin>,
    mesh: Arc<MeshHandle<SkinnedVertex>>,
    material: Asset<Material>,
    shader: Handle<Shader>,
    skinning_buffer: TypedBuffer<Mat4>,
    skinning_data: Vec<Mat4>,

    bind_group: BindGroup,
}

impl Batch {
    pub fn new(
        mesh: Arc<MeshHandle<SkinnedVertex>>,
        material: Asset<Material>,
        shader: Handle<Shader>,
        skin: Asset<Skin>,
        skinning_buffer: TypedBuffer<Mat4>,
        bind_group: BindGroup,
    ) -> Self {
        Self {
            instance_count: 0,
            first_instance: 0,
            mesh,
            material,
            shader,
            instance_capacity: 0,
            skin,
            skinning_buffer,
            skinning_data: Vec::new(),
            bind_group,
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
        render_pass.set_bind_group(first_bindgroup + 1, &self.bind_group, &[]);
        render_pass.set_bind_group(first_bindgroup + 2, self.material.bind_group(), &[]);

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
    transform: Copied<Component<Mat4>>,
    animator: Component<Animator>,
    skin: Component<Asset<Skin>>,
}

impl ObjectDataQuery {
    pub fn new() -> Self {
        Self {
            transform: world_transform().copied(),
            animator: animator(),
            skin: skin(),
        }
    }
}

pub struct SkinnedMeshRenderer {
    id: Entity,
    /// All the objects registered
    /// |****-|**---|**|
    ///
    /// Sorted by each batch
    object_data: Vec<ObjectData>,
    object_buffer: TypedBuffer<ObjectData>,

    bind_group_layout: BindGroupLayout,

    pub meshes: HashMap<MeshDesc, Weak<MeshHandle<SkinnedVertex>>>,
    pub shaders: AssetMap<crate::shader::ShaderPassDesc, Handle<Shader>>,
    pub materials: HashMap<MaterialDesc, Asset<Material>>,

    batches: Slab<Batch>,
    batch_map: HashMap<BatchKey, BatchId>,

    object_query: Query<(
        Component<usize>,
        Component<usize>,
        Source<ObjectDataQuery, Traverse>,
    )>,

    mesh_buffer: MeshBuffer<SkinnedVertex>,

    shader_pass: Component<Asset<ShaderPassDesc>>,
}

impl SkinnedMeshRenderer {
    pub fn new(
        world: &mut World,
        gpu: &Gpu,
        shader_pass: Component<Asset<ShaderPassDesc>>,
    ) -> Self {
        let id = world.spawn();

        let bind_group_layout = BindGroupLayoutBuilder::new("ObjectBuffer")
            .bind_storage_buffer(ShaderStages::VERTEX)
            .bind_storage_buffer(ShaderStages::VERTEX)
            .build(gpu);

        let object_buffer = TypedBuffer::new(
            gpu,
            "Object buffer",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            &[ObjectData::zeroed(); 64],
        );

        Self {
            id,
            object_data: Vec::new(),
            object_buffer,
            bind_group_layout,
            meshes: Default::default(),
            shaders: Default::default(),
            materials: Default::default(),
            batches: Default::default(),
            batch_map: Default::default(),
            object_query: Query::new((
                object_index(id),
                batch_id(id),
                ObjectDataQuery::new().traverse(mesh_primitive),
            )),
            mesh_buffer: MeshBuffer::new(gpu, "mesh_buffer", 4),
            shader_pass,
        }
    }

    fn resize_object_buffer(&mut self, gpu: &Gpu, capacity: usize) {
        if self.object_buffer.len() >= capacity {
            return;
        }

        self.object_buffer
            .resize(gpu, capacity.next_power_of_two(), false);
    }

    pub fn collect_unbatched(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        layouts: &[&BindGroupLayout],
        store: &mut RendererStore,
        cmd: &mut CommandBuffer,
        target: &TargetDesc,
    ) {
        let mut query = Query::new((
            entity_ids(),
            mesh().cloned(),
            material().cloned(),
            self.shader_pass.cloned(),
            skin().cloned().traverse(mesh_primitive),
        ))
        .without(object_index(self.id));

        let mut needs_reallocation = false;
        for (id, mesh, material, shader, skin) in &mut query.borrow(world) {
            let skinning_buffer = TypedBuffer::new_uninit(
                gpu,
                "skinning_buffer",
                BufferUsages::STORAGE | BufferUsages::COPY_DST,
                skin.joints().len() * 64,
            );

            let key = BatchKey {
                mesh,
                material,
                shader,
                skin,
            };

            let batch_id = *self.batch_map.entry(key).or_insert_with_key(|k| {
                let mut load_mesh = |v: &MeshDesc| {
                    let data = v.load_data(assets).unwrap();

                    Arc::new(self.mesh_buffer.insert(
                        gpu,
                        &data.skinned_vertices().collect_vec(),
                        data.indices(),
                    ))
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

                let material: Asset<Material> = assets.try_load(&k.material).unwrap_or_else(|e| {
                    tracing::error!(?k.material, "{:?}", e.context("Failed to load material"));
                    assets.load(&MaterialDesc::content(MaterialData::new()))
                });

                let shader = self.shaders.entry(&k.shader).or_insert_with(|| {
                    store.shaders.insert(Shader::new(
                        gpu,
                        &ShaderDesc {
                            label: k.shader.label(),
                            source: k.shader.source(),
                            vertex_layouts: &[SkinnedVertex::layout()],
                            layouts: &layouts
                                .iter()
                                .copied()
                                .chain([&self.bind_group_layout, material.layout()])
                                .collect_vec(),
                            vertex_entry_point: "vs_main",
                            fragment_entry_point: "fs_main",
                            target,
                        },
                    ))
                });

                let bind_group = BindGroupBuilder::new("ObjectBuffer")
                    .bind_buffer(&self.object_buffer)
                    .bind_buffer(&skinning_buffer)
                    .build(gpu, &self.bind_group_layout);

                self.batches.insert(Batch::new(
                    mesh,
                    material,
                    shader.clone(),
                    k.skin.clone(),
                    skinning_buffer,
                    bind_group,
                ))
            });

            cmd.set(id, object_index(self.id), usize::MAX).set(
                id,
                self::batch_id(self.id),
                batch_id,
            );

            let batch = &mut self.batches[batch_id];

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
            batch.skinning_data = vec![Mat4::IDENTITY; batch.skin.joints().len() * cap as usize];

            batch.bind_group = BindGroupBuilder::new("SkinnedObjectBuffer")
                .bind_buffer(&self.object_buffer)
                .bind_buffer(&batch.skinning_buffer)
                .build(gpu, &self.bind_group_layout);

            cursor += cap;
        }

        let mut query = Query::new((
            batch_id(self.id),
            object_index(self.id).as_mut(),
            ObjectDataQuery::new().traverse(mesh_primitive),
        ));

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
        for (&object_index, &batch_index, item) in &mut self.object_query.borrow(world) {
            let batch = &mut self.batches[batch_index];

            let local_offset = object_index - batch.first_instance as usize;

            self.object_data[object_index] = ObjectData {
                transform: item.transform,
            };

            item.animator
                .fill_buffer(item.skin, &mut batch.skinning_data[local_offset..]);
        }

        self.object_buffer.write(&gpu.queue, 0, &self.object_data);

        for (_, batch) in &self.batches {
            batch
                .skinning_buffer
                .write(&gpu.queue, 0, &batch.skinning_data);
        }
    }
}

impl CameraRenderer for SkinnedMeshRenderer {
    fn update(&mut self, ctx: &mut super::UpdateContext) -> anyhow::Result<()> {
        let mut cmd = CommandBuffer::new();
        self.collect_unbatched(
            ctx.world,
            ctx.assets,
            ctx.gpu,
            ctx.layouts,
            ctx.store,
            &mut cmd,
            &ctx.target_desc,
        );
        self.update_object_data(ctx.world, ctx.gpu);

        Ok(())
    }

    fn draw<'s>(
        &'s mut self,
        ctx: &'s super::RenderContext<'s>,
        render_pass: &mut RenderPass<'s>,
    ) -> anyhow::Result<()> {
        for (i, bind_group) in ctx.bind_groups.iter().enumerate() {
            render_pass.set_bind_group(i as _, bind_group, &[]);
        }

        self.object_buffer
            .write(&ctx.gpu.queue, 0, &self.object_data);

        tracing::trace!("drawing {} batches", self.batch_map.len());

        self.mesh_buffer.bind(render_pass);

        for batch_id in self.batch_map.values() {
            let batch = &self.batches[*batch_id];
            tracing::trace!(instance_count = batch.instance_count, "drawing batch");
            batch.draw(
                ctx.gpu,
                ctx.assets,
                ctx.store,
                render_pass,
                ctx.bind_groups.len() as _,
            )
        }

        Ok(())
    }
}

type BatchId = usize;

flax::component! {
    batch_id(id): BatchId,
    object_index(id): usize,
}
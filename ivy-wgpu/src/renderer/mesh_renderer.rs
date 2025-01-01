use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap},
    sync::{Arc, Weak},
};

use bytemuck::{Pod, Zeroable};
use flax::{
    entity_ids,
    fetch::entity_refs,
    filter::{All, ChangeFilter, With},
    Component, Entity, EntityIds, FetchExt, Query, World,
};
use itertools::Itertools;
use ivy_assets::{map::AssetMap, stored::Handle, Asset, AssetCache};
use ivy_core::{profiling::profile_function, subscribers::RemovedComponentSubscriber, WorldExt};
use ivy_wgpu_types::{shader::Culling, TypedBuffer};
use slab::Slab;
use wgpu::{BindGroupLayout, BufferUsages, DepthBiasState, RenderPass};

use super::{
    object_manager::{object_buffer_index, object_skinning_buffer},
    CameraRenderer, TargetDesc,
};
use crate::{
    components::mesh,
    material::RenderMaterial,
    material_desc::{MaterialData, PbrMaterialData, RenderMaterialDesc},
    mesh::{SkinnedVertex, VertexDesc},
    mesh_buffer::{MeshBuffer, MeshHandle},
    mesh_desc::MeshDesc,
    renderer::RendererStore,
    shader::ShaderPass,
    shader_library::ShaderLibrary,
    types::{shader::ShaderDesc, RenderShader},
    Gpu,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub material: MaterialData,
    pub mesh: MeshDesc,
}

/// A single rendering batch of similar objects
struct Batch {
    mesh: Arc<MeshHandle<SkinnedVertex>>,
    material: Asset<RenderMaterial>,
    shader: Handle<RenderShader>,
}

impl Batch {
    pub fn new(
        mesh: Arc<MeshHandle<SkinnedVertex>>,
        material: Asset<RenderMaterial>,
        shader: Handle<RenderShader>,
    ) -> Self {
        Self {
            mesh,
            material,
            shader,
        }
    }
}

pub type ShaderFactory = Box<dyn FnMut(ShaderDesc) -> ShaderDesc>;

struct DrawObject {
    id: Entity,
    object_index: usize,
    batch_id: usize,
}

struct IndirectBatch {
    batch_id: usize,
    offset: usize,
    count: usize,
}

/// Argument buffer layout for draw_indexed_indirect commands.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
pub struct DrawIndexedIndirectArgs {
    /// The number of indices to draw.
    pub index_count: u32,
    /// The number of instances to draw.
    pub instance_count: u32,
    /// The first index within the index buffer.
    pub first_index: u32,
    /// The value added to the vertex index before indexing into the vertex buffer.
    pub base_vertex: i32,
    /// The instance ID of the first instance to draw.
    ///
    /// Has to be 0, unless [`Features::INDIRECT_FIRST_INSTANCE`](crate::Features::INDIRECT_FIRST_INSTANCE) is enabled.
    pub first_instance: u32,
}

pub struct MeshRenderer {
    id: Entity,
    pub meshes: HashMap<MeshDesc, Weak<MeshHandle<SkinnedVertex>>>,
    pub shaders: AssetMap<ShaderPass, Handle<RenderShader>>,

    /// Keep track of loaded materials
    // TODO: move higher to deduplicate globally
    pub materials: HashMap<MaterialData, Asset<RenderMaterial>>,

    batches: Slab<Batch>,
    draws: Vec<DrawObject>,
    draw_map: BTreeMap<Entity, usize>,
    batch_map: HashMap<BatchKey, BatchId>,
    indirect_draws: Vec<IndirectBatch>,
    indirect_commands: TypedBuffer<DrawIndexedIndirectArgs>,

    mesh_buffer: MeshBuffer<SkinnedVertex>,
    shader_pass: Component<MaterialData>,
    shader_library: Arc<ShaderLibrary>,
    shader_factory: ShaderFactory,
    updated_object_indexes: Query<(EntityIds, ChangeFilter<usize>), (All, With)>,
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

        let (removed_tx, removed_rx) = flume::unbounded();
        world.subscribe(RemovedComponentSubscriber::new(
            removed_tx,
            renderer_location(id),
        ));

        let indirect_commands = TypedBuffer::new_uninit(
            gpu,
            "IndirectDraw",
            BufferUsages::INDIRECT | BufferUsages::COPY_DST,
            128,
        );

        Self {
            id,
            shader_library,
            meshes: Default::default(),
            shaders: Default::default(),
            materials: Default::default(),
            batches: Default::default(),
            batch_map: Default::default(),
            mesh_buffer: MeshBuffer::new(gpu, "mesh_buffer", 4),
            shader_pass,
            shader_factory: Box::new(|v| v),
            removed_rx,
            draws: Vec::new(),
            draw_map: Default::default(),
            updated_object_indexes: Query::new((entity_ids(), object_buffer_index().modified()))
                .with(renderer_location(id)),
            indirect_draws: Vec::new(),
            indirect_commands,
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

    #[allow(clippy::too_many_arguments)]
    pub fn process_new_objects(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        layouts: &[&BindGroupLayout],
        store: &mut RendererStore,
        target: &TargetDesc,
    ) -> anyhow::Result<()> {
        let mut query = Query::new((
            entity_refs(),
            mesh().cloned(),
            self.shader_pass.cloned(),
            object_buffer_index(),
            object_skinning_buffer().satisfied(),
        ))
        .without(renderer_location(self.id));
        let mut new_components = Vec::new();

        let mut new_objects = false;
        for (entity, mesh, material, &object_index, skinned) in &mut query.borrow(world) {
            let id = entity.id();
            let key = BatchKey { mesh, material };

            let mut create_batch = |key: &BatchKey| {
                let mut load_mesh = |v: &MeshDesc| {
                    let mesh_data = v.load_data(assets).unwrap();

                    Arc::new(self.mesh_buffer.insert(
                        gpu,
                        &SkinnedVertex::compose_from_mesh(&mesh_data),
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

                let material = RenderMaterialDesc {
                    material: key.material.clone(),
                    skinned,
                };

                let broken_material = |e: anyhow::Error| {
                    tracing::error!(?key.material, "{:?}", e.context("Failed to load material"));
                    assets.load(&RenderMaterialDesc {
                        material: MaterialData::PbrMaterial(PbrMaterialData::new()),
                        skinned: false,
                    })
                };

                let material: Asset<RenderMaterial> =
                    assets.try_load(&material).unwrap_or_else(broken_material);

                let shader = material.shader();
                let shader =
                    match self.shaders.entry(shader) {
                        slotmap::secondary::Entry::Occupied(slot) => slot.get().clone(),
                        slotmap::secondary::Entry::Vacant(slot) => {
                            let module = self.shader_library.process(gpu, (&**shader).into())?;

                            let vertex_layouts = &[SkinnedVertex::layout()];

                            let bind_group_layouts = layouts
                                .iter()
                                .copied()
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

            let draw = DrawObject {
                id,
                object_index,
                batch_id,
            };

            new_components.push((id, RendererLocation { batch_id }));

            self.draw_map.insert(id, self.draws.len());
            self.draws.push(draw);
            new_objects = true;
        }

        world
            .append_all(renderer_location(self.id), new_components)
            .unwrap();

        if !new_objects {
            return Ok(());
        }

        let mut total_object_count = 0;
        self.indirect_draws.clear();
        self.draws.sort_by_key(|v| v.batch_id);

        let chunks = self.draws.iter().chunk_by(|v| v.batch_id);
        for (batch_id, group) in &chunks {
            let group_count = group.count();
            let batch = IndirectBatch {
                batch_id,
                offset: total_object_count,
                count: group_count,
            };

            tracing::info!("group of {group_count}");
            self.indirect_draws.push(batch);
            total_object_count += group_count;
        }

        let indirect_draws = self
            .draws
            .iter()
            .map(|v| {
                let batch = &self.batches[v.batch_id];

                let index_offset = batch.mesh.ib().offset() as u32;
                let index_count = batch.mesh.index_count() as u32;

                DrawIndexedIndirectArgs {
                    index_count,
                    instance_count: 1,
                    first_index: index_offset,
                    base_vertex: 0,
                    first_instance: v.object_index as u32,
                }
            })
            .collect_vec();

        self.indirect_commands.write(&gpu.queue, 0, &indirect_draws);

        Ok(())
    }

    pub fn process_moved_objects(&mut self, world: &World) {
        for (id, &index) in self.updated_object_indexes.borrow(world).iter() {
            let draw_index = *self.draw_map.get(&id).unwrap();
            self.draws[draw_index].object_index = index;
        }
    }

    pub fn handle_removed(&mut self) {
        for (id, _) in self.removed_rx.try_iter() {
            let index = self.draw_map.remove(&id).expect("Object not in renderer");
            if index == self.draws.len() - 1 {
                self.draws.pop();
            } else {
                self.draws.swap_remove(index);
                let swapped = &self.draws[index];
                assert_eq!(self.draw_map.get(&swapped.id), Some(&self.draws.len()));
                *self.draw_map.get_mut(&swapped.id).unwrap() = index;
            }
        }
    }
}

impl CameraRenderer for MeshRenderer {
    fn update(&mut self, ctx: &mut super::UpdateContext) -> anyhow::Result<()> {
        profile_function!();
        self.process_new_objects(
            ctx.world,
            ctx.assets,
            ctx.gpu,
            ctx.layouts,
            ctx.store,
            &ctx.target_desc,
        )?;
        self.process_moved_objects(ctx.world);
        self.handle_removed();

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

        self.mesh_buffer.bind(render_pass);

        for draw in &self.indirect_draws {
            let batch = &self.batches[draw.batch_id];

            if let Some(bind_group) = batch.material.bind_group() {
                render_pass.set_bind_group(ctx.bind_groups.len() as u32, bind_group, &[]);
            }

            render_pass.set_pipeline(ctx.store.shaders[&batch.shader].pipeline());

            render_pass.multi_draw_indexed_indirect(
                &self.indirect_commands,
                draw.offset as u64 * size_of::<DrawIndexedIndirectArgs>() as u64,
                draw.count as u32,
            );
            // render_pass.draw_indexed(
            //     index_offset..index_offset + index_count,
            //     0,
            //     draw.object_index as u32..draw.object_index as u32 + 1,
            // );
        }

        Ok(())
    }
}

type BatchId = usize;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
struct RendererLocation {
    batch_id: BatchId,
}

flax::component! {
    renderer_location(id): RendererLocation,
}

use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap},
    sync::{Arc, Weak},
};

use bytemuck::{Pod, Zeroable};
use flax::{
    entity_ids,
    fetch::{entity_refs, EntityRefs, Satisfied},
    filter::{All, ChangeFilter},
    Component, Entity, EntityIds, FetchExt, Query, World,
};
use glam::{vec4, Mat4, Vec3, Vec4, Vec4Swizzles};
use itertools::Itertools;
use ivy_assets::{map::AssetMap, stored::Handle, Asset, AssetCache};
use ivy_core::{profiling::profile_function, subscribers::RemovedComponentSubscriber, WorldExt};
use ivy_wgpu_types::{
    multi_buffer::SubBuffer, shader::Culling, BindGroupBuilder, BindGroupLayoutBuilder,
};
use wgpu::{BindGroup, BindGroupLayout, CommandEncoder, DepthBiasState, RenderPass, ShaderStages};

use super::{
    culling::{CullDrawObject, ObjectCulling},
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
    renderer::{culling::CullData, RendererStore},
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
    mesh: CachedMesh,
    material: Asset<RenderMaterial>,
    shader: Handle<RenderShader>,
}

impl Batch {
    pub fn new(
        mesh: CachedMesh,
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

#[derive(Copy, Clone)]
struct IndirectBatch {
    batch_id: u32,
    offset: u32,
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

struct WeakCachedMesh {
    handle: Weak<MeshHandle<SkinnedVertex>>,
    bounding_radius: f32,
}

struct CachedMesh {
    handle: Arc<MeshHandle<SkinnedVertex>>,
    bounding_radius: f32,
}

type NewObjectQuery = (
    EntityRefs,
    Component<MeshDesc>,
    Component<MaterialData>,
    Component<usize>,
    Satisfied<Component<SubBuffer<Mat4>>>,
);

pub struct MeshRenderer {
    id: Entity,

    object_buffer_gen: u32,
    skin_buffer_gen: u32,
    bind_group: Option<BindGroup>,
    bind_group_layout: BindGroupLayout,
    meshes: HashMap<MeshDesc, WeakCachedMesh>,
    pub shaders: AssetMap<ShaderPass, Handle<RenderShader>>,

    /// Keep track of loaded materials
    // TODO: move higher to deduplicate globally
    pub materials: HashMap<MaterialData, Asset<RenderMaterial>>,

    batches: Vec<Batch>,
    draws: Vec<CullDrawObject>,
    sorted_draws: Vec<CullDrawObject>,
    entity_locations: BTreeMap<Entity, usize>,
    batch_map: HashMap<BatchKey, BatchId>,
    indirect_draws: Vec<DrawIndexedIndirectArgs>,
    indirect_batches: Vec<Option<IndirectBatch>>,

    mesh_buffer: MeshBuffer<SkinnedVertex>,
    shader_library: Arc<ShaderLibrary>,
    shader_factory: ShaderFactory,
    updated_object_indexes: Query<(EntityIds, Component<usize>, ChangeFilter<usize>)>,
    removed_rx: flume::Receiver<(Entity, usize)>,
    cull: ObjectCulling,
    new_object_query: Query<NewObjectQuery, (All, flax::filter::Without)>,
    needs_indirect_rebuild: bool,
}

impl MeshRenderer {
    pub fn new(
        world: &mut World,
        assets: &AssetCache,
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

        let cull = ObjectCulling::new(assets, gpu);

        let bind_group_layout = BindGroupLayoutBuilder::new("ObjectBuffer")
            .bind_storage_buffer(ShaderStages::VERTEX) // object_data
            .bind_storage_buffer(ShaderStages::VERTEX) // indirection
            .bind_storage_buffer(ShaderStages::VERTEX) // skin_data
            .build(gpu);

        let new_object_query = Query::new((
            entity_refs(),
            mesh(),
            shader_pass,
            object_buffer_index(),
            object_skinning_buffer().satisfied(),
        ))
        .without(renderer_location(id));

        Self {
            id,
            bind_group: None,
            bind_group_layout,
            cull,
            shader_library,
            meshes: Default::default(),
            shaders: Default::default(),
            materials: Default::default(),
            batches: Default::default(),
            batch_map: Default::default(),
            mesh_buffer: MeshBuffer::new(gpu, "mesh_buffer", 4),
            shader_factory: Box::new(|v| v),
            removed_rx,
            draws: Vec::new(),
            updated_object_indexes: Query::new((
                entity_ids(),
                renderer_location(id),
                object_buffer_index().modified(),
            )),
            new_object_query,
            indirect_draws: Vec::new(),
            indirect_batches: Vec::new(),
            object_buffer_gen: 0,
            skin_buffer_gen: 0,
            needs_indirect_rebuild: true,
            entity_locations: BTreeMap::new(),
            sorted_draws: Vec::new(),
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
        let mut new_components = Vec::new();

        for (entity, mesh, material, &object_index, skinned) in
            self.new_object_query.borrow(world).iter()
        {
            let id = entity.id();
            let key = BatchKey {
                mesh: mesh.clone(),
                material: material.clone(),
            };

            let mut create_batch = |key: &BatchKey| {
                let mut load_mesh = |v: &MeshDesc| {
                    let mesh_data = v.load_data(assets).unwrap();
                    let vertices = SkinnedVertex::compose_from_mesh(&mesh_data);

                    let bounding_radius = vertices
                        .iter()
                        .map(|v| v.pos.length())
                        .max_by_key(|&v| ordered_float::OrderedFloat(v))
                        .unwrap_or_default();

                    CachedMesh {
                        handle: Arc::new(self.mesh_buffer.insert(
                            gpu,
                            &vertices,
                            mesh_data.indices(),
                        )),
                        bounding_radius,
                    }
                };

                let mesh = match self.meshes.entry(key.mesh.clone()) {
                    Entry::Occupied(mut v) => {
                        if let Some(mesh) = v.get().handle.upgrade() {
                            CachedMesh {
                                handle: mesh,
                                bounding_radius: v.get().bounding_radius,
                            }
                        } else {
                            let mesh = load_mesh(v.key());
                            v.insert(WeakCachedMesh {
                                handle: Arc::downgrade(&mesh.handle),
                                bounding_radius: mesh.bounding_radius,
                            });

                            mesh
                        }
                    }
                    Entry::Vacant(v) => {
                        let mesh = load_mesh(v.key());
                        v.insert(WeakCachedMesh {
                            handle: Arc::downgrade(&mesh.handle),
                            bounding_radius: mesh.bounding_radius,
                        });

                        mesh
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
                    let batch_index = self.batches.len();
                    self.batches.push(batch?);
                    slot.insert(batch_index);
                    batch_index
                }
            };

            let draw = CullDrawObject {
                object_index: object_index as u32,
                batch_id: batch_id as u32,
                radius: self.batches[batch_id].mesh.bounding_radius,
                id,
            };

            let new_index = self.draws.len();
            new_components.push((id, new_index));
            self.entity_locations.insert(id, new_index);

            self.draws.push(draw);
            self.needs_indirect_rebuild = true;
        }

        world
            .append_all(renderer_location(self.id), new_components)
            .unwrap();

        Ok(())
    }

    fn rebuild_indirect_batches(&mut self, gpu: &Gpu) {
        let mut total_object_count = 0;
        self.indirect_draws.clear();
        self.indirect_batches.clear();
        self.indirect_batches.resize(self.batches.len(), None);
        self.indirect_draws.resize(
            self.batches.len(),
            DrawIndexedIndirectArgs {
                index_count: 0,
                instance_count: 0,
                first_index: 0,
                base_vertex: 0,
                first_instance: 0,
            },
        );

        self.sorted_draws.clear();
        self.sorted_draws.extend(self.draws.iter().copied());
        // sort same batches by id, to ensure stable rendering
        self.sorted_draws.sort_by_key(|v| (v.batch_id, v.id));

        let chunks = self.sorted_draws.iter().chunk_by(|v| v.batch_id);
        for (batch_id, group) in &chunks {
            let instance_count = group.count() as u32;
            let batch = &self.batches[batch_id as usize];
            let cmd = DrawIndexedIndirectArgs {
                index_count: batch.mesh.handle.index_count() as u32,
                instance_count: 0, // filled by culling
                first_index: batch.mesh.handle.ib().offset() as u32,
                base_vertex: 0,
                first_instance: total_object_count,
            };

            let batch = IndirectBatch {
                batch_id,
                offset: batch_id,
            };

            self.indirect_draws[batch_id as usize] = cmd;
            self.indirect_batches[batch_id as usize] = Some(batch);
            total_object_count += instance_count;
        }

        self.cull.update_objects(gpu, &self.sorted_draws);
        if self.cull.bind_group.is_none() {
            self.bind_group = None;
        }
    }

    pub fn process_moved_objects(&mut self, world: &World) {
        for (id, &loc, &new_index) in self.updated_object_indexes.borrow(world).iter() {
            assert_eq!(self.draws[loc].id, id);
            self.draws[loc].object_index = new_index as u32;
            self.needs_indirect_rebuild = true
        }
    }

    pub fn process_removed(&mut self, world: &World) {
        for (id, _) in self.removed_rx.try_iter() {
            self.needs_indirect_rebuild = true;

            let loc = self.entity_locations.remove(&id).unwrap();
            if loc == self.draws.len() - 1 {
                self.draws.pop();
            } else {
                let end = self.draws.len() - 1;
                self.draws.swap_remove(loc);

                let swapped = &self.draws[loc];

                self.entity_locations.insert(swapped.id, loc);
                let _ = world.update(swapped.id, renderer_location(self.id), |v| {
                    assert_eq!(*v, end);
                    *v = loc;
                });
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
        self.process_removed(ctx.world);

        if self.needs_indirect_rebuild {
            self.needs_indirect_rebuild = false;
            self.rebuild_indirect_batches(ctx.gpu);
        }

        Ok(())
    }

    fn before_draw(
        &mut self,
        ctx: &super::RenderContext,
        encoder: &mut CommandEncoder,
    ) -> anyhow::Result<()> {
        profile_function!();
        let object_buffer = ctx.object_manager.object_buffer();
        let skinning_buffer = ctx.object_manager.skinning_buffer();

        if self.object_buffer_gen != object_buffer.gen()
            || self.skin_buffer_gen != skinning_buffer.gen()
        {
            self.object_buffer_gen = object_buffer.gen();
            self.skin_buffer_gen = skinning_buffer.gen();

            self.bind_group = None;
            self.cull.bind_group = None;
        }

        fn normalize_plane(plane: Vec4) -> Vec4 {
            plane / plane.xyz().length()
        }

        fn transform_perspective(inv_viewproj: Mat4, clip: Vec3) -> Vec3 {
            let p = inv_viewproj * clip.extend(1.0);
            p.xyz() / p.w
        }

        let proj_transposed = ctx.camera.proj.transpose();
        let frustum_x = normalize_plane(proj_transposed.col(3) + proj_transposed.col(0));
        let frustum_y = normalize_plane(proj_transposed.col(3) + proj_transposed.col(1));
        let inv_proj = ctx.camera.proj.inverse();
        let near = -transform_perspective(inv_proj, Vec3::ZERO).z;
        let far = -transform_perspective(inv_proj, Vec3::Z).z;

        let cull_data = CullData {
            view: ctx.camera.view,
            frustum: vec4(frustum_x.x, frustum_x.z, frustum_y.y, frustum_y.z),
            near,
            far,
            object_count: self.draws.len() as u32,
            _padding: Default::default(),
        };

        self.cull
            .update_run_commands(ctx.gpu, cull_data, &self.indirect_draws);

        if self.cull.bind_group.is_none() {
            self.bind_group = None;
        }

        self.cull.run(ctx.gpu, encoder, cull_data, object_buffer);

        Ok(())
    }

    fn draw<'s>(
        &'s mut self,
        ctx: &'s super::RenderContext<'s>,
        render_pass: &mut RenderPass<'s>,
    ) -> anyhow::Result<()> {
        profile_function!();

        let object_buffer = ctx.object_manager.object_buffer();
        let skinning_buffer = ctx.object_manager.skinning_buffer();

        let bind_group = self.bind_group.get_or_insert_with(|| {
            BindGroupBuilder::new("ObjectBuffer")
                .bind_buffer(object_buffer.buffer())
                .bind_buffer(self.cull.indirection_buffer())
                .bind_buffer(skinning_buffer.buffer())
                .build(ctx.gpu, &self.bind_group_layout)
        });

        for (i, bind_group) in ctx.bind_groups.iter().enumerate() {
            render_pass.set_bind_group(i as _, bind_group, &[]);
        }

        render_pass.set_bind_group(ctx.bind_groups.len() as u32, bind_group, &[]);

        self.mesh_buffer.bind(render_pass);

        for draw in &self.indirect_batches {
            let Some(draw) = draw else {
                continue;
            };

            let batch = &self.batches[draw.batch_id as usize];

            if let Some(bind_group) = batch.material.bind_group() {
                render_pass.set_bind_group(ctx.bind_groups.len() as u32 + 1, bind_group, &[]);
            }

            render_pass.set_pipeline(ctx.store.shaders[&batch.shader].pipeline());

            render_pass.draw_indexed_indirect(
                self.cull.indirect_draw_buffer(),
                draw.offset as u64 * size_of::<DrawIndexedIndirectArgs>() as u64,
            );
        }

        Ok(())
    }
}

type BatchId = usize;

flax::component! {
    renderer_location(id): usize,
}

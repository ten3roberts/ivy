use hecs::{Entity, World};
use ivy_graphics::{Error, Material, Mesh, ShaderPass};
use ivy_resources::{Handle, ResourceCache};
use ivy_vulkan::{
    commands::CommandBuffer, descriptors::*, vk, Buffer, BufferAccess, BufferType, VulkanContext,
};
use std::{any::TypeId, collections::HashMap, marker::PhantomData, mem::size_of, sync::Arc};
use ultraviolet::Mat4;

use crate::{components::ModelMatrix, FRAMES_IN_FLIGHT};

/// Any entity with these components will be renderered.
type RenderObject<'a, T> = (
    &'a Handle<T>,
    &'a Handle<Mesh>,
    &'a Handle<Material>,
    &'a ModelMatrix,
    &'a ObjectBufferMarker,
);

/// Same as RenderObject except without ObjectBufferMarker
type RenderObjectUnregistered<'a, T> = (
    &'a Handle<T>,
    &'a Handle<Mesh>,
    &'a Handle<Material>,
    &'a ModelMatrix,
);

pub const MAX_OBJECTS: usize = 8096;

pub struct BatchedMeshRenderer {
    frames: Vec<FrameData>,
    passes: HashMap<TypeId, PassData>,
    max_object_id: usize,
    free_indices: Vec<usize>,
}

impl BatchedMeshRenderer {
    pub fn new(
        context: Arc<VulkanContext>,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        descriptor_allocator: &mut DescriptorAllocator,
    ) -> Result<Self, Error> {
        let frames = (0..FRAMES_IN_FLIGHT)
            .map(|_| {
                FrameData::new(
                    context.clone(),
                    descriptor_layout_cache,
                    descriptor_allocator,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let passes = HashMap::new();

        Ok(Self {
            frames,
            passes,
            max_object_id: 0,
            free_indices: Vec::new(),
        })
    }

    /// Inserts a new entity and return the marker
    fn insert_entity(&mut self, entity: Entity) -> (Entity, ObjectBufferMarker) {
        if let Some(id) = self.free_indices.pop() {
            (entity, ObjectBufferMarker::new(id))
        } else {
            let id = self.max_object_id;
            self.max_object_id += 1;
            (entity, ObjectBufferMarker::new(id))
        }
    }

    /// Registers all unregisters entities capable of being rendered for specified pass. Does
    /// nothing if entities are already registered.
    fn register_entities<T: 'static + ShaderPass + Send + Sync>(&mut self, world: &mut World) {
        let query = world
            .query_mut::<RenderObjectUnregistered<T>>()
            .without::<ObjectBufferMarker>();

        let inserted = query
            .into_iter()
            .map(|(e, _)| self.insert_entity(e))
            .collect::<Vec<_>>();

        inserted.into_iter().for_each(|(e, marker)| {
            world.insert_one(e, marker).unwrap();
        });
    }

    /// Updates all registered entities gpu side data
    pub fn update(&mut self, world: &mut World, current_frame: usize) -> Result<(), Error> {
        let query = world.query_mut::<(&ModelMatrix, &ObjectBufferMarker)>();

        let frame = &mut self.frames[current_frame];

        frame
            .object_buffer
            .write_slice(MAX_OBJECTS as u64, 0, |data| {
                query.into_iter().for_each(|(_, (modelmatrix, marker))| {
                    data[marker.id] = ObjectData { mvp: **modelmatrix }
                });
            })?;

        Ok(())
    }
    /// Will draw all entities with a Handle<Material>, Handle<Mesh>, Modelmatrix and Shaderpass `Handle<T>`
    pub fn draw<T: 'static + ShaderPass + Sized + Sync + Send>(
        &mut self,
        world: &mut World,
        cmd: &CommandBuffer,
        current_frame: usize,
        global_set: DescriptorSet,

        materials: &mut ResourceCache<Material>,
        meshes: &mut ResourceCache<Mesh>,
        passes: &mut ResourceCache<T>,
    ) -> Result<(), Error> {
        self.register_entities::<T>(world);

        let frame = &mut self.frames[current_frame];

        let frame_set = frame.set;

        let pass = self
            .passes
            .entry(TypeId::of::<T>())
            .or_insert_with(PassData::new);

        pass.build_batches::<T>(world, passes)?;

        pass.draw(cmd, global_set, frame_set, meshes, materials)?;

        Ok(())
    }
}

struct PassData {
    batches: Vec<BatchData>,
    batch_map: HashMap<BatchKey, usize>,
}

impl PassData {
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
            batch_map: HashMap::new(),
        }
    }

    /// Builds rendering batches for shaderpass `T` for all objects not yet batched.
    pub fn build_batches<T: 'static + ShaderPass + Send + Sync>(
        &mut self,
        world: &mut World,
        passes: &ResourceCache<T>,
    ) -> Result<(), Error> {
        let query = world
            .query_mut::<RenderObject<T>>()
            .without::<BatchMarker<T>>();

        let unbatched = query
            .into_iter()
            .map(|(e, renderobject)| self.insert_entity::<T>(e, renderobject, passes))
            .collect::<Result<Vec<_>, _>>()?;

        unbatched.into_iter().for_each(|(e, marker)| {
            world.insert_one(e, marker).unwrap();
        });

        Ok(())
    }

    pub fn draw(
        &mut self,
        cmd: &CommandBuffer,
        global_set: DescriptorSet,
        frame_set: DescriptorSet,
        meshes: &mut ResourceCache<Mesh>,
        materials: &mut ResourceCache<Material>,
    ) -> Result<(), Error> {
        for batch in &self.batches {
            let material = materials.get(batch.material)?;
            let mesh = meshes.get(batch.mesh)?;
            cmd.bind_descriptor_sets(
                batch.pipeline_layout,
                0,
                &[global_set, frame_set, material.set()],
            );

            cmd.bind_pipeline(batch.pipeline);
            cmd.bind_vertexbuffer(0, mesh.vertex_buffer());
            cmd.bind_indexbuffer(mesh.index_buffer(), 0);

            let index_count = mesh.index_count();

            for index in &batch.ids {
                cmd.draw_indexed(index_count, 1, 0, 0, *index as u32);
            }
        }

        Ok(())
    }

    /// Inserts a new entity into the correct. Note: The entity should not already exist in pass,
    /// behaviour is undefined.
    pub fn insert_entity<'a, T: 'static + ShaderPass>(
        &mut self,
        entity: Entity,
        renderobject: RenderObject<'a, T>,
        passes: &ResourceCache<T>,
    ) -> Result<(Entity, BatchMarker<T>), Error> {
        let (shaderpass, mesh, material, _modelmatrix, object_marker) = renderobject;

        let shaderpass = passes.get(*shaderpass)?;
        let (_batch_idx, batch) = self.get_batch(shaderpass, *mesh, *material);

        batch.ids.push(object_marker.id);

        Ok((
            entity,
            BatchMarker {
                _shaderpass: PhantomData,
            },
        ))
    }

    pub fn get_batch<T: ShaderPass>(
        &mut self,
        shaderpass: &T,
        mesh: Handle<Mesh>,
        material: Handle<Material>,
    ) -> (usize, &mut BatchData) {
        let idx = match self
            .batch_map
            .get(&(shaderpass.pipeline().into(), mesh, material))
        {
            Some(val) => *val,
            None => {
                self.batches
                    .push(BatchData::new(shaderpass, mesh, material));
                self.batch_map.insert(
                    (shaderpass.pipeline().into(), mesh, material),
                    self.batches.len() - 1,
                );
                self.batches.len() - 1
            }
        };

        (idx, &mut self.batches[idx])
    }
}

pub type BatchKey = (vk::Pipeline, Handle<Mesh>, Handle<Material>);

/// A batch contains objects of the same shaderpass and material.
struct BatchData {
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    material: Handle<Material>,
    mesh: Handle<Mesh>,
    /// The number of draw calls in batch
    /// Indices into the object buffer for objects to draw
    ids: Vec<usize>,
}

impl BatchData {
    fn new<T: ShaderPass>(shaderpass: &T, mesh: Handle<Mesh>, material: Handle<Material>) -> Self {
        Self {
            pipeline: shaderpass.pipeline().into(),
            pipeline_layout: shaderpass.pipeline_layout(),
            material,
            mesh,
            ids: Vec::new(),
        }
    }
}

struct ObjectBufferMarker {
    /// Index into the object buffer
    id: usize,
}

impl ObjectBufferMarker {
    fn new(index: usize) -> Self {
        Self { id: index }
    }
}

/// Marks the entity as already being batched for this shaderpasss with the batch index and object buffer index.
struct BatchMarker<T> {
    _shaderpass: PhantomData<T>,
}

struct FrameData {
    set: DescriptorSet,
    object_buffer: Buffer,
}

impl FrameData {
    pub fn new(
        context: Arc<VulkanContext>,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        descriptor_allocator: &mut DescriptorAllocator,
    ) -> Result<Self, Error> {
        let object_buffer = Buffer::new_uninit(
            context.clone(),
            BufferType::Storage,
            BufferAccess::MappedPersistent,
            (size_of::<ObjectData>() * MAX_OBJECTS) as u64,
        )?;

        let mut set = Default::default();
        let mut set_layout = Default::default();

        DescriptorBuilder::new()
            .bind_storage_buffer(0, vk::ShaderStageFlags::VERTEX, &object_buffer)
            .build(
                context.device(),
                descriptor_layout_cache,
                descriptor_allocator,
                &mut set,
            )?
            .layout(descriptor_layout_cache, &mut set_layout)?;

        Ok(Self { set, object_buffer })
    }
}

#[repr(C)]
struct ObjectData {
    mvp: Mat4,
}

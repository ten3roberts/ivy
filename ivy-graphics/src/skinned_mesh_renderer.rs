use crate::{
    Allocator, Animator, BaseRenderer, BatchMarker, BufferAllocation, Error, Material, Renderer,
    Result, Skin, SkinnedMesh,
};
use ash::vk::{DescriptorSet, IndexType, ShaderStageFlags};
use glam::{Mat4, Vec4};
use hecs::{Query, World};
use hecs_schedule::CommandBuffer;
use ivy_base::{Color, Position, Rotation, Scale, TransformMatrix, Visible};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    context::SharedVulkanContext, descriptors::IntoSet, device, shaderpass::ShaderPass, Buffer,
    BufferUsage,
};
use smallvec::SmallVec;
use std::iter::repeat;

/// A mesh renderer using vkCmdDrawIndirectIndexed and efficient batching.
pub struct SkinnedMeshRenderer {
    base_renderer: BaseRenderer<Key, ObjectData>,
    frames_in_flight: usize,
    /// Buffers containing joint transforms
    buffers: SmallVec<[(Buffer, DescriptorSet); 3]>,
    allocator: Allocator<Marker>,
    cmd: CommandBuffer,
}

impl SkinnedMeshRenderer {
    pub fn new(
        context: SharedVulkanContext,
        capacity: u32,
        joint_capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let base_renderer = BaseRenderer::new(context.clone(), capacity, frames_in_flight)?;

        let buffers =
            Self::create_joint_buffers(context.clone(), joint_capacity, frames_in_flight)?;

        Ok(Self {
            base_renderer,
            allocator: Allocator::new(capacity as _),
            buffers,
            frames_in_flight,
            cmd: CommandBuffer::new(),
        })
    }

    fn create_joint_buffers(
        context: SharedVulkanContext,
        joint_capacity: u32,
        frames_in_flight: usize,
    ) -> Result<SmallVec<[(Buffer, DescriptorSet); 3]>> {
        (0..frames_in_flight)
            .map(|_| {
                let buffer = Buffer::new_iter(
                    context.clone(),
                    BufferUsage::STORAGE_BUFFER,
                    ivy_vulkan::BufferAccess::Mapped,
                    joint_capacity as _,
                    repeat(TransformMatrix::default()),
                )?;

                let set = ivy_vulkan::descriptors::DescriptorBuilder::new()
                    .bind_buffer(0, ShaderStageFlags::VERTEX, &buffer)?
                    .build(&context)?;

                Ok((buffer, set))
            })
            .collect()
    }

    /// Registers all unregistered entities capable of being rendered for specified pass. Does
    /// nothing if entities are already registered. Call this function after adding new entities to the world.
    /// # Failures
    /// Fails if object buffer cannot be reallocated to accomodate new entities.
    fn register_entities(&mut self, world: &mut World, resources: &Resources) -> Result<()> {
        let skins = resources.fetch::<Skin>()?;

        let needs_resize = world
            .query_mut::<&Handle<Skin>>()
            .without::<BufferAllocation<Marker>>()
            .into_iter()
            .try_for_each(|(e, skin)| {
                let skin = skins.get(*skin).unwrap();
                let block = self.allocator.allocate(skin.joint_count())?;
                self.cmd.insert_one(e, block);
                Some(())
            })
            .is_none();

        self.cmd.execute(world);

        if needs_resize {
            self.grow()?;

            return self.register_entities(world, resources);
        }

        Ok(())
    }

    fn update_joints(
        &mut self,
        world: &mut World,
        resources: &Resources,
        current_frame: usize,
    ) -> Result<()> {
        let skins = resources.fetch::<Skin>()?;
        self.buffers[current_frame]
            .0
            .write_slice(
                self.allocator.capacity() as _,
                0,
                |data: &mut [TransformMatrix]| {
                    world
                        .query_mut::<(&mut Animator, &Handle<Skin>, &BufferAllocation<Marker>)>()
                        .into_iter()
                        .for_each(|(_, (animator, skin, block))| {
                            let slice = &mut data[block.offset()..block.offset() + block.len()];
                            let skin = skins.get(*skin).unwrap();
                            let root = skin.root();
                            animator.fill_sparse(skin, slice, root, TransformMatrix::default());
                        })
                },
            )
            .map_err(|e| e.into())
    }

    fn grow(&mut self) -> Result<()> {
        let context = self.base_renderer.context();
        device::wait_idle(context.device())?;

        self.allocator.grow_double();

        self.buffers = Self::create_joint_buffers(
            context.clone(),
            self.allocator.capacity() as _,
            self.frames_in_flight,
        )?;

        Ok(())
    }
}

impl Drop for SkinnedMeshRenderer {
    fn drop(&mut self) {
        self.base_renderer.context().wait_idle().unwrap();
    }
}

impl Renderer for SkinnedMeshRenderer {
    type Error = Error;
    /// Will draw all entities with a Handle<Material>, Handle<Mesh>, Modelmatrix and Shaderpass `Handle<T>`
    fn draw<Pass: ShaderPass>(
        &mut self,
        // The ecs world
        world: &mut World,
        // The commandbuffer to record into
        cmd: &ivy_vulkan::CommandBuffer,
        // The current swapchain image or backbuffer index
        current_frame: usize,
        // Descriptor sets to bind before renderer specific sets
        sets: &[DescriptorSet],
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // Graphics resources like textures and materials
        resources: &Resources,
    ) -> Result<()> {
        let passes = resources.fetch::<Pass>()?;
        let meshes = resources.fetch::<SkinnedMesh>()?;
        let materials = resources.fetch::<Material>()?;

        self.register_entities(world, resources)?;
        self.update_joints(world, resources, current_frame)?;

        let pass = self.base_renderer.pass_mut::<Pass>()?;

        pass.get_unbatched::<Pass, KeyQuery, ObjectDataQuery>(world);
        pass.build_batches::<Pass, KeyQuery>(world, &*passes)?;

        let iter = world
            .query_mut::<(&BatchMarker<ObjectData, Pass>, ObjectDataQuery, &Visible)>()
            .into_iter()
            .filter_map(|(e, (marker, obj, visible))| {
                if visible.is_visible() {
                    Some((e, (marker, obj)))
                } else {
                    None
                }
            });

        pass.update(current_frame, iter)?;

        let frame_set = pass.set(current_frame);
        let joint_set = self.buffers[current_frame].1;

        for batch in pass.batches() {
            let key = batch.key();

            let mesh = meshes.get(key.mesh)?;

            cmd.bind_pipeline(batch.pipeline());

            if !sets.is_empty() {
                cmd.bind_descriptor_sets(batch.layout(), 0, sets, offsets);
            }

            cmd.bind_vertexbuffer(0, mesh.vertex_buffer());
            cmd.bind_indexbuffer(mesh.index_buffer(), IndexType::UINT32, 0);

            let primitives = mesh.primitives();
            let instance_count = batch.instance_count();
            let first_instance = batch.first_instance();

            if !key.material.is_null() {
                let material = materials.get(key.material)?;
                cmd.bind_descriptor_sets(
                    batch.layout(),
                    sets.len() as u32,
                    &[frame_set, material.set(current_frame), joint_set],
                    &[],
                );
                cmd.draw_indexed(mesh.index_count(), instance_count, 0, 0, first_instance);
            } else if !primitives.is_empty() {
                primitives.iter().try_for_each(|val| -> Result<()> {
                    let material = materials.get(val.material)?;

                    cmd.bind_descriptor_sets(
                        batch.layout(),
                        sets.len() as u32,
                        &[frame_set, material.set(current_frame), joint_set],
                        &[],
                    );

                    cmd.draw_indexed(
                        val.index_count,
                        instance_count,
                        val.first_index,
                        0,
                        first_instance,
                    );

                    Ok(())
                })?;
            }
        }

        Ok(())
    }
}

#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct ObjectData {
    model: Mat4,
    color: Vec4,
    offset: u32,
    len: u32,
    pad: [f32; 2],
}

#[derive(Query)]
struct ObjectDataQuery<'a> {
    position: &'a Position,
    rotation: &'a Rotation,
    scale: &'a Scale,
    color: Option<&'a Color>,
    block: &'a BufferAllocation<Marker>,
}

impl<'a> Into<ObjectData> for ObjectDataQuery<'a> {
    fn into(self) -> ObjectData {
        ObjectData {
            model: Mat4::from_translation(**self.position)
                * self.rotation.into_matrix()
                * Mat4::from_scale(**self.scale),
            color: self.color.cloned().unwrap_or(Color::white()).into(),
            offset: self.block.offset() as u32,
            len: self.block.len() as u32,
            pad: Default::default(),
        }
    }
}

#[derive(Query, PartialEq, Eq)]
struct KeyQuery<'a> {
    mesh: &'a Handle<SkinnedMesh>,
    material: Option<&'a Handle<Material>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    mesh: Handle<SkinnedMesh>,
    material: Handle<Material>,
}

impl<'a> crate::KeyQuery for KeyQuery<'a> {
    type K = Key;

    fn into_key(&self) -> Self::K {
        Self::K {
            mesh: *self.mesh,
            material: self.material.cloned().unwrap_or_default(),
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
struct Marker;

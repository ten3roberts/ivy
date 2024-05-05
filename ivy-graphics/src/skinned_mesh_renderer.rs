use crate::{
    batch_id,
    components::{animator, material, mesh, skin, skinned_mesh},
    Allocator, Animator, BaseRenderer, BatchMarker, BufferAllocation, Error, Material, Renderer,
    Result, Skin, SkinnedMesh, SkinnedVertex,
};
use ash::vk::{DescriptorSet, IndexType, ShaderStageFlags};
use flax::{entity_ids, CommandBuffer, Component, Fetch, FetchExt, Opt, OptOr, Query, World};
use glam::{Mat4, Vec4};
use ivy_assets::{Asset, AssetCache};
use ivy_base::{color, Color, ColorExt, TransformQuery, Visible};
use ivy_vulkan::{
    context::SharedVulkanContext, descriptors::IntoSet, device, Buffer, BufferUsage, PassInfo,
    Shader,
};
use smallvec::SmallVec;
use std::iter::repeat;

/// A mesh renderer using vkCmdDrawIndirectIndexed and efficient batching.
pub struct SkinnedMeshRenderer {
    base_renderer: BaseRenderer<Key, ObjectData, SkinnedVertex>,
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
                    repeat(Mat4::IDENTITY),
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
    fn register_entities(&mut self, world: &mut World, assets: &AssetCache) -> Result<()> {
        let needs_resize = Query::new((entity_ids(), skin()))
            .without(joint_buffer())
            .borrow(world)
            .iter()
            .try_for_each(|(id, skin)| {
                let block = self.allocator.allocate(skin.joint_count())?;
                self.cmd.set(id, joint_buffer(), block);
                Some(())
            })
            .is_none();

        self.cmd.apply(world).unwrap();

        if needs_resize {
            self.grow()?;

            return self.register_entities(world, assets);
        }

        Ok(())
    }

    fn update_joints(&mut self, world: &mut World, current_frame: usize) -> Result<()> {
        self.buffers[current_frame]
            .0
            .write_slice(self.allocator.capacity() as _, 0, |data: &mut [Mat4]| {
                Query::new((animator().as_mut(), skin(), joint_buffer()))
                    .borrow(world)
                    .iter()
                    .for_each(|(animator, skin, block)| {
                        let slice = &mut data[block.offset()..block.offset() + block.len()];
                        for root in skin.roots() {
                            animator.fill_sparse(skin, slice, *root, Mat4::default());
                        }
                    });
                // world
                //     .query_mut::<(&mut Animator, &Handle<Skin>, &BufferAllocation<Marker>)>()
                //     .into_iter()
                //     .for_each(|(_, (animator, skin, block))| {
                //     })
            })
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
    /// Will draw all entities with a Handle<Material>, Handle<Mesh>, Modelmatrix and Shaderpass `Handle<T>`
    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        cmd: &ivy_vulkan::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
        pass: Component<Shader>,
    ) -> anyhow::Result<()> {
        return Ok(());

        self.register_entities(world, assets)?;
        self.update_joints(world, current_frame)?;

        let renderpass = self.base_renderer.pass_mut(pass)?;

        renderpass.register(world, KeyQuery::new());
        renderpass.build_batches(world, pass_info)?;

        renderpass.update(
            current_frame,
            Query::new((entity_ids(), batch_id(pass.id()), ObjectDataQuery::new()))
                .borrow(world)
                .iter()
                .filter_map(|(e, &batch_id, obj /* , bound */)| {
                    // if visible.is_visible()
                    //     && camera.visible(**obj.position, **bound * obj.scale.max_element())
                    // {
                    Some((e, batch_id, ObjectData::from(obj)))
                    // } else {
                    //     None
                    // }
                }),
        )?;

        let frame_set = renderpass.set(current_frame);
        let joint_set = self.buffers[current_frame].1;

        for batch in renderpass.batches().iter() {
            let key = batch.key();

            let mesh = key.mesh;

            cmd.bind_pipeline(batch.pipeline());

            if !sets.is_empty() {
                cmd.bind_descriptor_sets(batch.layout(), 0, sets, offsets);
            }

            cmd.bind_vertexbuffer(0, mesh.vertex_buffer());
            cmd.bind_indexbuffer(mesh.index_buffer(), IndexType::UINT32, 0);

            let primitives = mesh.primitives();
            let instance_count = batch.instance_count();
            let first_instance = batch.first_instance();

            if let Some(material) = key.material {
                cmd.bind_descriptor_sets(
                    batch.layout(),
                    sets.len() as u32,
                    &[frame_set, material.set(current_frame), joint_set],
                    &[],
                );
                cmd.draw_indexed(mesh.index_count(), instance_count, 0, 0, first_instance);
            } else if !primitives.is_empty() {
                primitives.iter().try_for_each(|val| -> Result<()> {
                    cmd.bind_descriptor_sets(
                        batch.layout(),
                        sets.len() as u32,
                        &[frame_set, val.material.set(current_frame), joint_set],
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

#[derive(Fetch)]
struct ObjectDataQuery {
    transform: TransformQuery,
    color: OptOr<Component<Color>, Color>,
    block: Component<BufferAllocation<Marker>>,
}

impl ObjectDataQuery {
    fn new() -> Self {
        Self {
            transform: TransformQuery::new(),
            color: color().opt_or(Color::new(1.0, 1.0, 1.0, 1.0)),
            block: joint_buffer(),
        }
    }
}

impl From<ObjectDataQueryItem<'_>> for ObjectData {
    fn from(value: ObjectDataQueryItem<'_>) -> ObjectData {
        ObjectData {
            model: Mat4::from_scale_rotation_translation(
                *value.transform.scale,
                *value.transform.rotation,
                *value.transform.pos,
            ),
            color: value.color.to_vec4(),
            offset: value.block.offset() as u32,
            len: value.block.len() as u32,
            pad: Default::default(),
        }
    }
}

#[derive(Fetch)]
struct KeyQuery {
    mesh: Component<Asset<SkinnedMesh>>,
    material: Opt<Component<Asset<Material>>>,
}

impl KeyQuery {
    fn new() -> Self {
        Self {
            mesh: skinned_mesh(),
            material: material().opt(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    mesh: Asset<SkinnedMesh>,
    material: Option<Asset<Material>>,
}

impl From<KeyQueryItem<'_>> for Key {
    fn from(value: KeyQueryItem<'_>) -> Self {
        Self {
            mesh: value.mesh.clone(),
            material: value.material.cloned(),
        }
    }
}

flax::component! {
    joint_buffer: BufferAllocation<Marker>,
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
struct Marker;

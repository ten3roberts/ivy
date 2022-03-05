use crate::{BaseRenderer, BatchMarker, Error, Material, Mesh, Renderer, Result, Vertex};
use ash::vk::{DescriptorSet, IndexType};
use glam::{Mat4, Vec4};
use hecs::{Query, World};
use ivy_base::{Color, Position, Rotation, Scale, Visible};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    context::SharedVulkanContext, descriptors::IntoSet, shaderpass::ShaderPass, PassInfo,
};

/// A mesh renderer using vkCmdDrawIndirectIndexed and efficient batching.
pub struct MeshRenderer {
    base_renderer: BaseRenderer<Key, ObjectData, Vertex>,
}

impl MeshRenderer {
    pub fn new(
        context: SharedVulkanContext,
        capacity: u32,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let base_renderer = BaseRenderer::new(context, capacity, frames_in_flight)?;

        Ok(Self { base_renderer })
    }
}

impl Renderer for MeshRenderer {
    type Error = Error;
    /// Will draw all entities with a Handle<Material>, Handle<Mesh>, Modelmatrix and Shaderpass `Handle<T>`
    fn draw<Pass: ShaderPass>(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &ivy_vulkan::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
    ) -> Result<()> {
        let meshes = resources.fetch::<Mesh>()?;
        let materials = resources.fetch::<Material>()?;

        let pass = self.base_renderer.pass_mut::<Pass>()?;

        pass.register::<Pass, KeyQuery, ObjectDataQuery>(world);
        pass.build_batches::<Pass, KeyQuery>(world, resources, pass_info)?;
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

        for batch in pass.batches().iter() {
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
                    &[frame_set, material.set(current_frame)],
                    &[],
                );
                cmd.draw_indexed(mesh.index_count(), instance_count, 0, 0, first_instance);
            } else if !primitives.is_empty() {
                primitives.iter().try_for_each(|val| -> Result<()> {
                    let material = materials.get(val.material)?;

                    cmd.bind_descriptor_sets(
                        batch.layout(),
                        sets.len() as u32,
                        &[frame_set, material.set(current_frame)],
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

#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[repr(C)]
struct ObjectData {
    model: Mat4,
    color: Vec4,
}

#[derive(Query)]
struct ObjectDataQuery<'a> {
    position: &'a Position,
    rotation: &'a Rotation,
    scale: &'a Scale,
    color: Option<&'a Color>,
}

impl<'a> Into<ObjectData> for ObjectDataQuery<'a> {
    fn into(self) -> ObjectData {
        ObjectData {
            model: Mat4::from_translation(**self.position)
                * Mat4::from_mat3(self.rotation.into_matrix3())
                * Mat4::from_scale(**self.scale),
            color: self.color.cloned().unwrap_or(Color::white()).into(),
        }
    }
}

#[derive(Query, PartialEq, Eq)]
struct KeyQuery<'a> {
    mesh: &'a Handle<Mesh>,
    material: Option<&'a Handle<Material>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    mesh: Handle<Mesh>,
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

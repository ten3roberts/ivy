use crate::{
    batch_id,
    components::{camera, material, mesh},
    BaseRenderer, BatchMarker, BoundingSphere, Camera, Error, MainCamera, Material, Mesh, Renderer,
    Result, Vertex,
};
use ash::vk::{DescriptorSet, IndexType};
use flax::{entity_ids, Component, Fetch, FetchExt, Opt, OptOr, Query, World};
use glam::{Mat4, Quat, Vec3, Vec4};
use ivy_base::{color, main_camera, position, rotation, scale, Color, ColorExt, Visible};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{context::SharedVulkanContext, descriptors::IntoSet, PassInfo, Shader};

/// Draw a static mesh with a material
pub struct MeshRenderer {
    base_renderer: BaseRenderer<MeshKey, ObjectData, Vertex>,
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
    /// Will draw all entities with a Handle<Material>, Handle<Mesh>, Modelmatrix and Shaderpass `Handle<T>`
    fn draw(
        &mut self,
        world: &mut World,
        resources: &Resources,
        cmd: &ivy_vulkan::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        current_frame: usize,
        pass: Component<Shader>,
    ) -> anyhow::Result<()> {
        let meshes = resources.fetch::<Mesh>()?;
        let materials = resources.fetch::<Material>()?;

        let renderpass = self.base_renderer.pass_mut(pass)?;

        renderpass.build_batches(world, resources, pass_info)?;
        {
            let mut q = Query::new(camera()).with(main_camera());
            let camera = q.borrow(world).iter().next().unwrap();

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
        }

        renderpass.register(world, KeyQuery::new());

        renderpass.build_batches(world, resources, pass_info)?;

        let frame_set = renderpass.set(current_frame);

        for batch in renderpass
            .batches()
            .iter()
            .filter(|v| v.instance_count() != 0)
        {
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

            assert_ne!(instance_count, 0);

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

#[derive(Fetch)]
struct ObjectDataQuery {
    position: Component<Vec3>,
    rotation: Component<Quat>,
    scale: Component<Vec3>,
    color: OptOr<Component<Color>, Color>,
}

impl ObjectDataQuery {
    fn new() -> Self {
        Self {
            position: position(),
            rotation: rotation(),
            scale: scale(),
            color: color().opt_or(Color::new(1.0, 1.0, 1.0, 1.0)),
        }
    }
}

impl From<ObjectDataQueryItem<'_>> for ObjectData {
    fn from(value: ObjectDataQueryItem) -> Self {
        Self {
            model: Mat4::from_scale_rotation_translation(
                *value.scale,
                *value.rotation,
                *value.position,
            ),
            color: value.color.to_vec4(),
        }
    }
}

#[derive(Fetch)]
#[fetch(item_derives = [ PartialEq, Eq ])]
struct KeyQuery {
    mesh: Component<Handle<Mesh>>,
    material: OptOr<Component<Handle<Material>>, Handle<Material>>,
}

impl KeyQuery {
    fn new() -> Self {
        Self {
            mesh: mesh(),
            material: material().opt_or_default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MeshKey {
    mesh: Handle<Mesh>,
    material: Handle<Material>,
}

impl From<KeyQueryItem<'_>> for MeshKey {
    fn from(value: KeyQueryItem<'_>) -> Self {
        Self {
            mesh: *value.mesh,
            material: *value.material,
        }
    }
}

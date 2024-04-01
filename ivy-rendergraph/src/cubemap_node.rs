use crate::{AttachmentInfo, CameraNode, CameraNodeInfo, Node};
use flax::{Component, Entity, World};
use glam::{Quat, Vec3};
use ivy_base::{connection, position, rotation, scale};
use ivy_graphics::{Camera, Renderer};
use ivy_resources::{Handle, Resources, Storage};
use ivy_vulkan::{
    context::SharedVulkanContext, descriptors::MultiDescriptorBindable, CubeMap, Shader,
};

pub fn setup_cubemap_node<R>(
    context: SharedVulkanContext,
    world: &mut World,
    resources: &Resources,
    renderer: Handle<R>,
    origin: Entity,
    camera: Camera,
    cubemap: Handle<CubeMap>,
    depth: Handle<CubeMap>,
    bindables: &[&dyn MultiDescriptorBindable],
    frames_in_flight: usize,
    shaderpass: Component<Shader>,
) -> crate::Result<Vec<Box<dyn Node>>>
where
    R: Renderer + Storage,
{
    let cubemap = resources.get(cubemap).unwrap();
    let depth = resources.get(depth).unwrap();
    // Create cameras
    let dirs = [Vec3::Z, Vec3::Z, Vec3::Y, Vec3::Y, Vec3::X, Vec3::X];

    cubemap
        .views()
        .iter()
        .zip(depth.views())
        .zip(dirs)
        .map(|((view, depth), dir)| -> crate::Result<Box<dyn Node>> {
            let camera = Entity::builder()
                .set_default(position())
                .set(rotation(), Quat::from_rotation_arc(Vec3::Y, dir))
                .set_default(scale())
                .set(ivy_graphics::components::camera(), camera.clone())
                .set_default(connection(origin))
                .spawn(world);

            let node = CameraNode::new(
                context.clone(),
                world,
                resources,
                camera,
                renderer,
                shaderpass,
                CameraNodeInfo {
                    name: "cube_map_camera_node",
                    color_attachments: vec![AttachmentInfo::color(*view)],
                    read_attachments: &[],
                    input_attachments: vec![],
                    depth_attachment: Some(AttachmentInfo::depth_discard(*depth)),
                    bindables,
                    frames_in_flight,
                    ..Default::default()
                },
            )?;

            Ok(Box::new(node))
        })
        .collect()
}

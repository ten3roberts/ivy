use crate::{AttachmentInfo, CameraNode, CameraNodeInfo, Node};
use glam::Vec3;
use hecs::{Entity, World};
use hecs_hierarchy::HierarchyMut;
use ivy_base::{Connection, Position, Rotation, Scale};
use ivy_graphics::{Camera, Renderer};
use ivy_resources::{Handle, Resources, Storage};
use ivy_vulkan::{
    context::SharedVulkanContext, descriptors::MultiDescriptorBindable, CubeMap, ShaderPass,
};

pub fn setup_cubemap_node<P, R>(
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
) -> crate::Result<Vec<Box<dyn Node>>>
where
    P: ShaderPass + Storage,
    R: Renderer + Storage,
    R::Error: Into<anyhow::Error> + Send,
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
            let camera = world
                .attach_new::<Connection, _>(
                    origin,
                    (
                        Position::default(),
                        Rotation::look_at(dir, Vec3::Y),
                        Scale::default(),
                        camera.clone(),
                    ),
                )
                .unwrap();

            let node = CameraNode::<P, _>::new(
                context.clone(),
                resources,
                camera,
                renderer,
                CameraNodeInfo {
                    name: "Cube map camera node",
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

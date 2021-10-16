use std::sync::Arc;

use ash::vk::{DescriptorSet, IndexType, ShaderStageFlags};
use ivy_core::Gizmos;
use ivy_vulkan::VulkanContext;
use ultraviolet::{Mat4, Vec3, Vec4};

use crate::{Mesh, Renderer, Result};

pub struct GizmoRenderer {
    mesh: crate::Mesh,
}

impl GizmoRenderer {
    pub fn new(context: Arc<VulkanContext>) -> Result<Self> {
        let mesh = Mesh::new_square(context, 2.0, 2.0)?;

        Ok(Self { mesh })
    }
}

impl Renderer for GizmoRenderer {
    type Error = crate::Error;

    fn draw<Pass: crate::ShaderPass>(
        &mut self,
        // The ecs world
        _: &mut hecs::World,
        // The commandbuffer to record into
        cmd: &ivy_vulkan::commands::CommandBuffer,
        // The current swapchain image or backbuffer index
        _: usize,
        // Descriptor sets to bind before renderer specific sets
        sets: &[DescriptorSet],
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // Graphics resources like textures and materials
        resources: &ivy_resources::Resources,
    ) -> Result<()> {
        cmd.bind_vertexbuffer(0, self.mesh.vertex_buffer());
        cmd.bind_indexbuffer(self.mesh.index_buffer(), IndexType::UINT32, 0);

        let shaderpass = resources.get_default::<Pass>()?;
        let layout = shaderpass.pipeline_layout();

        let gizmos = resources.get_default::<Gizmos>()?;

        cmd.bind_pipeline(shaderpass.pipeline());

        if !sets.is_empty() {
            cmd.bind_descriptor_sets(layout, 0, sets, offsets);
        }

        for gizmo in gizmos.iter() {
            match gizmo {
                ivy_core::Gizmo::Sphere {
                    origin,
                    color,
                    radius,
                } => {
                    cmd.push_constants(
                        layout,
                        ShaderStageFlags::VERTEX,
                        0,
                        &PushConstantData {
                            model: Mat4::from_translation(*origin) * Mat4::from_scale(*radius),
                            color: **color,
                            billboard_axis: Vec4::zero(),
                        },
                    );

                    cmd.draw_indexed(6, 1, 0, 0, 0);
                }
                ivy_core::Gizmo::Line {
                    origin,
                    color,
                    dir,
                    radius,
                } => {
                    // let a = dir.cross(Vec3::new(-dir.y, dir.x, dir.z)).normalized() * *radius;
                    // let b = a.cross(*dir).normalized() * *radius;

                    cmd.push_constants(
                        layout,
                        ShaderStageFlags::VERTEX,
                        0,
                        &PushConstantData {
                            model: Mat4::from_translation(*origin + *dir * 0.5)
                                * Mat4::from_nonuniform_scale(Vec3::new(
                                    *radius,
                                    dir.mag() * 0.5,
                                    *radius,
                                )),
                            color: **color,
                            billboard_axis: dir.normalized().into_homogeneous_vector(),
                        },
                    );

                    cmd.draw_indexed(6, 1, 0, 0, 0);
                }
                ivy_core::Gizmo::Cube {
                    origin,
                    color,
                    half_extents,
                    radius,
                } => todo!(),
            }
        }

        Ok(())

        // cmd.draw_indexed(6, batch.instance_count(), 0, 0, batch.first_instance());
    }
}

#[repr(C)]
struct PushConstantData {
    model: Mat4,
    color: Vec4,
    billboard_axis: Vec4,
}

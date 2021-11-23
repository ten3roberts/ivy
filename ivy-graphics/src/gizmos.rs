use std::sync::Arc;

use ash::vk::{DescriptorSet, IndexType, ShaderStageFlags};
use ivy_base::Gizmos;
use ivy_vulkan::{shaderpass::ShaderPass, VulkanContext};
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

    fn draw<Pass: ShaderPass>(
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

        let gizmos = resources
            .default_entry::<Gizmos>()?
            .or_insert_with(|| Gizmos::default());

        cmd.bind_pipeline(shaderpass.pipeline());

        if !sets.is_empty() {
            cmd.bind_descriptor_sets(layout, 0, sets, offsets);
        }

        for gizmo in gizmos.sections().iter().flat_map(|val| val.1) {
            match gizmo {
                ivy_base::Gizmo::Sphere {
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
                            color: color.into(),
                            billboard_axis: Vec3::zero(),
                            corner_radius: 1.0,
                        },
                    );

                    cmd.draw_indexed(6, 1, 0, 0, 0);
                }
                ivy_base::Gizmo::Line {
                    origin,
                    color,
                    dir,
                    radius,
                    corner_radius,
                } => {
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
                            color: color.into(),
                            billboard_axis: dir.normalized(),
                            corner_radius: *corner_radius,
                        },
                    );

                    cmd.draw_indexed(6, 1, 0, 0, 0);
                }
                ivy_base::Gizmo::Cube {
                    origin,
                    color,
                    half_extents,
                    radius,
                } => {
                    for (v, dir) in [
                        (Vec3::unit_x(), Vec3::unit_z()),
                        (Vec3::unit_y(), Vec3::unit_x()),
                        (Vec3::unit_z(), Vec3::unit_y()),
                    ] {
                        for (a, b) in [(1.0, 1.0), (-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0)] {
                            cmd.push_constants(
                                layout,
                                ShaderStageFlags::VERTEX,
                                0,
                                &PushConstantData {
                                    model: Mat4::from_translation(
                                        *origin
                                            + b * (Vec3::one() - (dir + v) + a * v) * *half_extents,
                                    ) * Mat4::from_nonuniform_scale(Vec3::new(
                                        *radius,
                                        (dir * (*half_extents)).mag() + radius * 1.0,
                                        *radius,
                                    )),
                                    color: color.into(),
                                    billboard_axis: dir.normalized(),
                                    corner_radius: 1.0,
                                },
                            );

                            cmd.draw_indexed(6, 1, 0, 0, 0);
                        }
                    }
                }
                ivy_base::Gizmo::Triangle {
                    color,
                    points,
                    radius,
                } => {
                    for i in [(0, 1), (0, 2), (1, 2)] {
                        let (p1, p2) = (points[i.0], points[i.1]);
                        let dir = p2 - p1;
                        cmd.push_constants(
                            layout,
                            ShaderStageFlags::VERTEX,
                            0,
                            &PushConstantData {
                                model: Mat4::from_translation(p1 + dir * 0.5)
                                    * Mat4::from_nonuniform_scale(Vec3::new(
                                        *radius,
                                        dir.mag() * 0.5,
                                        *radius,
                                    )),
                                color: color.into(),
                                billboard_axis: dir.normalized(),
                                corner_radius: 1.0,
                            },
                        );

                        cmd.draw_indexed(6, 1, 0, 0, 0);
                    }
                }
            }
        }

        Ok(())
    }
}

#[repr(C)]
struct PushConstantData {
    model: Mat4,
    color: Vec4,
    billboard_axis: Vec3,
    corner_radius: f32,
}

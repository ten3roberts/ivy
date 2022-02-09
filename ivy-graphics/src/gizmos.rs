use ash::vk::{DescriptorSet, IndexType, ShaderStageFlags};
use glam::{Mat4, Vec3, Vec4};
use hecs::World;
use ivy_base::Gizmos;
use ivy_resources::Resources;
use ivy_vulkan::{context::SharedVulkanContext, shaderpass::ShaderPass, PassInfo, Pipeline};
use once_cell::sync::OnceCell;

use crate::{Mesh, Renderer, Result, Vertex};

pub struct GizmoRenderer {
    mesh: crate::Mesh,
    pipeline: OnceCell<Pipeline>,
}

impl GizmoRenderer {
    pub fn new(context: SharedVulkanContext) -> Result<Self> {
        let mesh = Mesh::new_square(context, 2.0, 2.0)?;

        Ok(Self {
            mesh,
            pipeline: OnceCell::new(),
        })
    }
}

impl Renderer for GizmoRenderer {
    type Error = crate::Error;

    fn draw<Pass: ShaderPass>(
        &mut self,
        _: &mut World,
        resources: &Resources,
        cmd: &ivy_vulkan::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        _: usize,
    ) -> Result<()> {
        let pipeline = self.pipeline.get_or_try_init(|| {
            let context = resources.get_default::<SharedVulkanContext>()?;
            let pass = resources.get_default::<Pass>()?;
            Pipeline::new::<Vertex>(context.clone(), pass.pipeline(), pass_info)
        })?;

        cmd.bind_vertexbuffer(0, self.mesh.vertex_buffer());
        cmd.bind_indexbuffer(self.mesh.index_buffer(), IndexType::UINT32, 0);

        let layout = pipeline.layout();

        let gizmos = resources
            .default_entry::<Gizmos>()?
            .or_insert_with(|| Gizmos::default());

        cmd.bind_pipeline(pipeline);

        if !sets.is_empty() {
            cmd.bind_descriptor_sets(layout, 0, sets, offsets);
        }

        for gizmo in gizmos.sections().iter().flat_map(|val| val.1) {
            match gizmo {
                ivy_base::GizmoPrimitive::Sphere {
                    origin,
                    color,
                    radius,
                } => {
                    cmd.push_constants(
                        layout,
                        ShaderStageFlags::VERTEX,
                        0,
                        &PushConstantData {
                            model: Mat4::from_translation(*origin)
                                * Mat4::from_scale(Vec3::splat(*radius)),
                            color: color.into(),
                            billboard_axis: Vec3::ZERO,
                            corner_radius: 1.0,
                        },
                    );

                    cmd.draw_indexed(6, 1, 0, 0, 0);
                }
                ivy_base::GizmoPrimitive::Line {
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
                                * Mat4::from_scale(Vec3::new(*radius, dir.length() * 0.5, *radius)),
                            color: color.into(),
                            billboard_axis: dir.normalize(),
                            corner_radius: *corner_radius,
                        },
                    );

                    cmd.draw_indexed(6, 1, 0, 0, 0);
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

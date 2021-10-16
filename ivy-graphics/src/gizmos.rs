use std::sync::Arc;

use ash::vk::{DescriptorSet, IndexType, ShaderStageFlags};
use ivy_core::Gizmos;
use ivy_vulkan::VulkanContext;
use ultraviolet::{Mat4, Vec4};

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
            let data = PushConstantData {
                model: Mat4::from_translation(gizmo.midpoint())
                    * Mat4::from_nonuniform_scale(gizmo.size()),
                color: gizmo.color(),
                billboard_axis: gizmo.billboard_axis().into_homogeneous_vector(),
            };

            cmd.push_constants(layout, ShaderStageFlags::VERTEX, 0, &data);

            cmd.draw_indexed(6, 1, 0, 0, 0);
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

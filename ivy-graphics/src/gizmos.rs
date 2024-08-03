use ash::vk::{DescriptorSet, IndexType, ShaderStageFlags};
use flax::{Component, World};
use glam::{Mat4, Vec3, Vec4};
use ivy_assets::AssetCache;
use ivy_core::{engine, gizmos, ColorExt};
use ivy_vulkan::{
    context::{SharedVulkanContext, VulkanContextService},
    PassInfo, Pipeline, PipelineInfo, Shader,
};
use once_cell::sync::OnceCell;

use crate::{Mesh, Renderer, Result, Vertex};

/// Renders all the drawn gizmos
/// TODO: supply the pipeline info/shader from a node
pub struct GizmoRenderer {
    mesh: crate::Mesh,
    pipeline: OnceCell<Pipeline>,
    pipeline_info: PipelineInfo,
}

impl GizmoRenderer {
    pub fn new(context: SharedVulkanContext, pipeline_info: PipelineInfo) -> Result<Self> {
        let mesh = Mesh::new_square(context, 2.0, 2.0)?;

        Ok(Self {
            mesh,
            pipeline: OnceCell::new(),
            pipeline_info,
        })
    }
}

impl Renderer for GizmoRenderer {
    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        cmd: &ivy_vulkan::CommandBuffer,
        sets: &[DescriptorSet],
        pass_info: &PassInfo,
        offsets: &[u32],
        _: usize,
        _: Component<Shader>,
    ) -> anyhow::Result<()> {
        let pipeline = self.pipeline.get_or_try_init(|| {
            let context = assets.service::<VulkanContextService>().context();
            Pipeline::new::<Vertex>(context.clone(), &self.pipeline_info, pass_info)
        })?;

        cmd.bind_vertexbuffer(0, self.mesh.vertex_buffer());
        cmd.bind_indexbuffer(self.mesh.index_buffer(), IndexType::UINT32, 0);

        let layout = pipeline.layout();

        let gizmos = world.get(engine(), gizmos())?;

        cmd.bind_pipeline(pipeline);

        if !sets.is_empty() {
            cmd.bind_descriptor_sets(layout, 0, sets, offsets);
        }

        unimplemented!();

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

use crate::{Mesh, Renderer, Result};
use ash::vk::{DescriptorSet, IndexType, ShaderStageFlags};
use derive_more::*;
use ivy_vulkan::VulkanContext;
use std::sync::Arc;
use ultraviolet::{Mat4, Vec3, Vec4};

#[derive(Copy, Clone, PartialEq)]
pub struct Gizmo {
    pos: Vec3,
    color: Vec4,
    kind: GizmoKind,
}

impl Gizmo {
    pub fn new(pos: Vec3, color: Vec4, kind: GizmoKind) -> Self {
        Self { pos, color, kind }
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub enum GizmoKind {
    Sphere(f32),
}

impl GizmoKind {
    pub fn scale(&self) -> Vec3 {
        match *self {
            Self::Sphere(r) => Vec3::new(r, r, r),
        }
    }
}

#[derive(Default, Deref, DerefMut)]
pub struct Gizmos(Vec<Gizmo>);

impl Gizmos {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

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
                model: Mat4::from_translation(gizmo.pos)
                    * Mat4::from_nonuniform_scale(gizmo.kind.scale()),
                color: gizmo.color,
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
}

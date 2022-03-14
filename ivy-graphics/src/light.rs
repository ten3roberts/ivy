use crate::{
    icosphere::create_ico_mesh,
    shaders::{LIGHT_VERTEX_SHADER, PBR_SHADER},
    MainCamera, Mesh, Renderer, Result, SimpleVertex,
};
use ash::vk::{BlendFactor, CullModeFlags, DescriptorSet, IndexType, ShaderStageFlags};
use glam::Vec3;
use hecs::World;
use ivy_base::{Position, WorldExt};
use ivy_vulkan::{
    context::SharedVulkanContext,
    descriptors::{DescriptorBuilder, IntoSet},
    Buffer, Pipeline, PipelineInfo,
};
use once_cell::sync::OnceCell;
use ordered_float::OrderedFloat;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight {
    pub radiance: Vec3,
    // Visible radius of the light source
    pub radius: f32,
}

impl PointLight {
    /// Creates a new light from color radience
    pub fn new(radius: f32, radiance: Vec3) -> Self {
        Self { radius, radiance }
    }

    /// Creates a light from color and intensity
    pub fn from_color(radius: f32, intensity: f32, color: Vec3) -> Self {
        Self {
            radius,
            radiance: intensity * color,
        }
    }
}

pub struct LightRenderer {
    scene_buffers: Vec<Buffer>,
    light_buffers: Vec<Buffer>,
    sets: Vec<DescriptorSet>,
    // All registered lights. Note: not all lights may be uploaded to the GPU
    lights: Vec<LightData>,

    max_lights: u64,
    num_lights: u64,
    sphere: Mesh<SimpleVertex>,
    pipeline: OnceCell<Pipeline>,
}

impl LightRenderer {
    pub fn new(
        context: SharedVulkanContext,
        max_lights: u64,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let scene_buffers = (0..frames_in_flight)
            .map(|_| -> Result<_> {
                Buffer::new_uninit::<LightSceneData>(
                    context.clone(),
                    ivy_vulkan::BufferUsage::UNIFORM_BUFFER,
                    ivy_vulkan::BufferAccess::Mapped,
                    1,
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        let light_buffers = (0..frames_in_flight)
            .map(|_| -> Result<_> {
                Buffer::new_uninit::<LightData>(
                    context.clone(),
                    ivy_vulkan::BufferUsage::STORAGE_BUFFER,
                    ivy_vulkan::BufferAccess::Mapped,
                    max_lights,
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        let sets = DescriptorBuilder::from_mutliple_resources(
            &context,
            &[(&light_buffers, ShaderStageFlags::VERTEX)],
            frames_in_flight,
        )?;

        let sphere = create_ico_mesh(&context, 1.0, 1)?;

        Ok(Self {
            pipeline: OnceCell::new(),
            scene_buffers,
            sphere,
            light_buffers,
            sets,
            lights: Vec::new(),
            num_lights: 0,
            max_lights,
        })
    }

    /// Updates the GPU side data of the world lights.
    /// Each light which has a [`PointLight`] and [`Position`] will be considered.
    /// The lights will be sorted in reference to centered. If there are more lights than `max_lights`,
    /// then the n closest will be used.
    pub fn update_system(
        &mut self,
        world: &World,
        center: Position,
        current_frame: usize,
    ) -> Result<()> {
        // let v = &self.sphere.0;
        // for tri in self.sphere.1.chunks_exact(3) {
        //     for edge in tri.windows(2) {
        //         Line::from_points(v[edge[0] as usize], v[edge[1] as usize], 0.01, 1.0)
        //             .draw_gizmos(gizmos, Color::yellow());
        //     }
        //     // v.draw_gizmos(gizmos, Color::red())
        // }
        // v.draw_gizmos(gizmos, Color::red())
        self.lights.clear();
        self.lights
            .extend(world.query::<(&PointLight, &Position)>().iter().map(
                |(_, (light, position))| LightData {
                    position: **position,
                    radiance: light.radiance,
                    size: (light.radiance.length() * 256.).sqrt(),
                    radius: light.radius,
                    ..Default::default()
                },
            ));

        self.lights
            .sort_unstable_by_key(|val| -OrderedFloat(val.position.distance_squared(*center)));

        self.num_lights = self.max_lights.min(self.lights.len() as u64);

        // Use the first `max_lights` lights and upload to gpu
        self.light_buffers[current_frame].fill(0, &self.lights[0..self.num_lights as usize])?;

        self.scene_buffers[current_frame].fill(
            0,
            &[LightSceneData {
                num_lights: self.num_lights as u32,
            }],
        )?;

        Ok(())
    }

    pub fn update_all_system(world: &World, current_frame: usize) -> Result<()> {
        world
            .query::<(&mut LightRenderer, &Position)>()
            .iter()
            .try_for_each(|(_, (light_manager, position))| {
                light_manager.update_system(world, *position, current_frame)
            })
    }

    pub fn scene_buffers(&self) -> &[Buffer] {
        &self.scene_buffers
    }

    pub fn light_buffers(&self) -> &[Buffer] {
        &self.light_buffers
    }

    pub fn scene_buffer(&self, current_frame: usize) -> &Buffer {
        &self.scene_buffers[current_frame]
    }

    pub fn light_buffer(&self, current_frame: usize) -> &Buffer {
        &self.light_buffers[current_frame]
    }
}

impl Renderer for LightRenderer {
    type Error = crate::Error;

    fn draw<Pass: ivy_vulkan::ShaderPass>(
        &mut self,
        // The ecs world
        world: &mut World,
        // Graphics resources like textures and materials
        resources: &ivy_resources::Resources,
        // The commandbuffer to record into
        cmd: &ivy_vulkan::CommandBuffer,
        // Descriptor sets to bind before renderer specific sets
        sets: &[DescriptorSet],
        // Information about the current pass
        pass_info: &ivy_vulkan::PassInfo,
        // Dynamic offsets for supplied sets
        offsets: &[u32],
        // The current swapchain image or backbuffer index
        current_frame: usize,
    ) -> Result<()> {
        let cam = world.by_tag::<MainCamera>().unwrap();
        let center = world.get::<Position>(cam)?;
        self.update_system(world, *center, current_frame)?;
        let pipeline = self.pipeline.get_or_try_init(|| {
            let context = resources.get_default::<SharedVulkanContext>()?;
            Pipeline::new::<SimpleVertex>(
                context.clone(),
                &PipelineInfo {
                    blending: true,
                    depth_clamp: true,
                    vs: LIGHT_VERTEX_SHADER,
                    src_color: BlendFactor::ONE,
                    dst_color: BlendFactor::ONE,
                    fs: PBR_SHADER,
                    cull_mode: CullModeFlags::BACK,
                    ..Default::default()
                },
                pass_info,
            )
        })?;

        cmd.bind_pipeline(pipeline);
        cmd.bind_descriptor_sets(pipeline.layout(), 0, sets, offsets);
        cmd.bind_descriptor_sets(
            pipeline.layout(),
            sets.len() as u32,
            &[self.sets[current_frame]],
            &[],
        );

        cmd.bind_indexbuffer(self.sphere.index_buffer(), IndexType::UINT32, 0);
        cmd.bind_vertexbuffer(0, self.sphere.vertex_buffer());

        cmd.draw_indexed(
            self.sphere.index_count(),
            self.num_lights.min(self.max_lights) as u32,
            0,
            0,
            0,
        );

        Ok(())
    }
}

impl IntoSet for LightRenderer {
    fn set(&self, current_frame: usize) -> DescriptorSet {
        self.sets[current_frame]
    }

    fn sets(&self) -> &[DescriptorSet] {
        &self.sets
    }
}

/// Per light data
#[repr(C, align(16))]
#[derive(Default, PartialEq, Debug)]
struct LightData {
    position: Vec3,
    size: f32,
    radiance: Vec3,
    radius: f32,
}

impl std::cmp::Eq for LightData {}

#[repr(C)]
struct LightSceneData {
    num_lights: u32,
}

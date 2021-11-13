use crate::Result;
use ash::vk::{DescriptorSet, ShaderStageFlags};
use derive_more::{AsRef, Deref, From, Into};
use hecs::World;
use ivy_base::Extent;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    descriptors::{DescriptorBuilder, IntoSet},
    Buffer, Format, ImageUsage, SampleCountFlags, Texture, TextureInfo, VulkanContext,
};
use std::sync::Arc;
use ultraviolet::{Mat4, Vec2, Vec3, Vec4};

/// A camera holds a view and projection matrix.
/// Use a system to update view matrix according to position and rotation.
pub struct Camera {
    projection: Mat4,
    view: Mat4,
    /// Cached version of view * projection
    viewproj: Mat4,
}

impl Camera {
    /// Creates a new camera with identity projection and view matrices.
    pub fn new() -> Self {
        Self {
            projection: Mat4::identity(),
            view: Mat4::identity(),
            viewproj: Mat4::identity(),
        }
    }

    /// Sets the camera to use a orthographic projection matrix.
    pub fn set_orthographic(&mut self, width: f32, height: f32, near: f32, far: f32) {
        let hw = width / 2.0;
        let hh = height / 2.0;

        self.projection = ultraviolet::projection::orthographic_vk(-hw, hw, -hh, hh, near, far);
        self.update_viewproj();
    }

    pub fn set_perspective(&mut self, fov: f32, aspect: f32, near: f32, far: f32) {
        self.projection = ultraviolet::projection::perspective_vk(fov, aspect, near, far);
        self.update_viewproj();
    }

    pub fn orthographic(width: f32, height: f32, near: f32, far: f32) -> Self {
        let mut camera = Camera::new();
        camera.set_orthographic(width, height, near, far);
        camera
    }

    pub fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        let mut camera = Camera::new();
        camera.set_perspective(fov, aspect, near, far);
        camera
    }

    fn update_viewproj(&mut self) {
        self.viewproj = self.projection * self.view;
    }

    /// Returns the cached combined view and projection matrix
    pub fn viewproj(&self) -> Mat4 {
        self.projection * self.view
    }

    /// Return the camera's projection matrix.
    pub fn projection(&self) -> Mat4 {
        self.projection
    }

    /// Set the camera's projection matrix.
    pub fn set_projection(&mut self, projection: Mat4) {
        self.projection = projection;
        self.update_viewproj();
    }

    /// Return the camera's view matrix.
    pub fn view(&self) -> Mat4 {
        self.view
    }

    /// Set the camera's view.
    pub fn set_view(&mut self, view: Mat4) {
        self.view = view;
        self.update_viewproj();
    }

    /// Transform a position in normalized device coordinates to a world
    /// direction eminating from the camera.
    pub fn to_world_ray(&self, pos: Vec2) -> Vec3 {
        let ray_clip = Vec4::new(pos.x, -pos.y, -1.0, 1.0);
        let ray_eye = self.projection.inversed() * ray_clip;
        (self.view.inversed() * Vec4::new(ray_eye.x, ray_eye.y, -1.0, 0.0))
            .xyz()
            .normalized()
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C, align(16))]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
/// GPU side camera data
pub struct CameraData {
    pub viewproj: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
    pub position: Vec4,
}

#[derive(AsRef, Deref, Into, From)]
/// The color attachment of a camera.
pub struct ColorAttachment(pub Handle<Texture>);

impl ColorAttachment {
    pub fn new(texture: Handle<Texture>) -> ColorAttachment {
        Self(texture)
    }
}

/// The depth attachment of a camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deref, AsRef, Into, From)]
pub struct DepthAttachment(pub Handle<Texture>);

impl DepthAttachment {
    pub fn new(context: Arc<VulkanContext>, resources: &Resources, extent: Extent) -> Result<Self> {
        Ok(Self(resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT
                    | ImageUsage::SAMPLED
                    | ImageUsage::INPUT_ATTACHMENT,
                format: Format::D32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?))
    }

    pub fn from_handle(texture: Handle<Texture>) -> DepthAttachment {
        Self(texture)
    }
}

pub struct GpuCameraData {
    uniformbuffers: Vec<Buffer>,
    sets: Vec<DescriptorSet>,
}

impl GpuCameraData {
    pub fn new(context: Arc<VulkanContext>, frames_in_flight: usize) -> Result<Self> {
        let uniformbuffers = (0..frames_in_flight)
            .map(|_| {
                Buffer::new(
                    context.clone(),
                    ivy_vulkan::BufferUsage::UNIFORM_BUFFER,
                    ivy_vulkan::BufferAccess::Mapped,
                    &[CameraData::default()],
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        let sets = uniformbuffers
            .iter()
            .map(|u| {
                DescriptorBuilder::new()
                    .bind_buffer(0, ShaderStageFlags::VERTEX, u)?
                    .build(&context)
                    .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            uniformbuffers,
            sets,
        })
    }

    /// Get a reference to the g p u camera data's uniformbuffers.
    pub fn buffers(&self) -> &[Buffer] {
        &self.uniformbuffers
    }

    pub fn buffer(&self, index: usize) -> &Buffer {
        &self.uniformbuffers[index]
    }

    // Updates the camera gpu side data from cpu side data for the current frame.
    pub fn update(&mut self, camera: &Camera, current_frame: usize) -> Result<()> {
        let view = camera.view();
        let position = view.inversed()[3].xyz().into_homogeneous_point();

        self.uniformbuffers[current_frame]
            .fill(
                0,
                &[CameraData {
                    viewproj: camera.viewproj(),
                    view,
                    projection: camera.projection(),
                    position,
                }],
            )
            .map_err(|e| e.into())
    }

    /// Updates all GPU camera data from the CPU side camera view and projection
    /// matrix. Position is automatically extracted from the camera's view matrix.
    pub fn update_all_system(world: &mut World, current_frame: usize) -> Result<()> {
        world
            .query_mut::<(&Camera, &mut GpuCameraData)>()
            .into_iter()
            .try_for_each(|(_, (camera, gpu_camera))| gpu_camera.update(camera, current_frame))
    }

    // Creates gpu side data for all camera which do not already have any.
    pub fn create_gpu_cameras(
        context: &Arc<VulkanContext>,
        world: &mut World,
        frames_in_flight: usize,
    ) -> Result<()> {
        let cameras = world
            .query_mut::<&Camera>()
            .without::<GpuCameraData>()
            .into_iter()
            .map(|val| val.0)
            .collect::<Vec<_>>();

        cameras.into_iter().try_for_each(|camera| -> Result<()> {
            let gpu_camera = GpuCameraData::new(context.clone(), frames_in_flight)?;
            world.insert_one(camera, gpu_camera).map_err(|e| e.into())
        })
    }
}

impl IntoSet for GpuCameraData {
    fn set(&self, current_frame: usize) -> DescriptorSet {
        self.sets[current_frame]
    }

    fn sets(&self) -> &[DescriptorSet] {
        &self.sets
    }
}

/// Narke
pub struct MainCamera;

use ash::vk::{DescriptorSet, ShaderStageFlags};
use derive_more::{AsRef, Deref, From, Into};
use flax::{entity_ids, BoxedSystem, Component, Mutable, Query, QueryBorrow, System, World};
use glam::{vec3, Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache};
use ivy_core::{Color, DrawGizmos, Extent, Line, Sphere};
use ivy_vulkan::{
    context::SharedVulkanContext,
    descriptors::{DescriptorBuilder, IntoSet},
    Buffer, Texture, TextureInfo,
};

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

use crate::{
    components::{camera, gpu_camera},
    Result,
};

/// A camera holds a view and projection matrix.
/// Use a system to update view matrix according to position and rotation.
#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Camera {
    projection: Mat4,
    view: Mat4,
    /// Cached version of view * projection
    viewproj: Mat4,
    frustum: Frustum,
}

impl Camera {
    /// Creates a new camera with identity projection and view matrices.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the camera to use a orthographic projection matrix.
    pub fn set_orthographic(&mut self, width: f32, height: f32, near: f32, far: f32) {
        let hw = width / 2.0;
        let hh = height / 2.0;

        self.projection = orthographic_vk(-hw, hw, -hh, hh, near, far);
        self.frustum = Frustum::ortho(hh, hw, near, far);
        self.update_viewproj();
    }

    pub fn set_perspective(&mut self, fov: f32, aspect: f32, near: f32, far: f32) {
        self.projection = perspective_vk(fov, aspect, near, far);
        self.frustum = Frustum::perspective(fov, aspect, near, far);
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
        let ray_eye = self.projection.inverse() * ray_clip;
        (self.view.inverse() * Vec4::new(ray_eye.x, ray_eye.y, -1.0, 0.0))
            .xyz()
            .normalize()
    }

    pub fn visible(&self, p: Vec3, radius: f32) -> bool {
        let p = self.view().transform_point3(p);
        self.frustum.visible(p, radius)
    }

    /// Get a reference to the camera's frustum.
    #[must_use]
    pub fn frustum(&self) -> &Frustum {
        &self.frustum
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
    pub forward: Vec4,
}

#[derive(AsRef, Deref, Into, From)]
/// The color attachment of a camera.
pub struct ColorAttachment(pub Asset<Texture>);

impl ColorAttachment {
    pub fn new(texture: Asset<Texture>) -> ColorAttachment {
        Self(texture)
    }
}

/// The depth attachment of a camera.
#[derive(Clone, PartialEq, Eq, Deref, AsRef, Into, From)]
pub struct DepthAttachment(pub Asset<Texture>);

impl DepthAttachment {
    pub fn new(context: SharedVulkanContext, assets: &AssetCache, extent: Extent) -> Result<Self> {
        Ok(Self(assets.insert(Texture::new(
            context.clone(),
            &TextureInfo::depth(extent),
        )?)))
    }

    pub fn from_handle(texture: Asset<Texture>) -> DepthAttachment {
        Self(texture)
    }
}

pub struct GpuCamera {
    uniformbuffers: Vec<Buffer>,
    sets: Vec<DescriptorSet>,
}

impl GpuCamera {
    pub fn new(context: SharedVulkanContext, frames_in_flight: usize) -> Result<Self> {
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
        let position = view.inverse().col(3).xyz();

        // tracing::info!(?camera, %current_frame, "updating gpu camera");
        self.uniformbuffers[current_frame]
            .fill(
                0,
                &[CameraData {
                    viewproj: camera.viewproj(),
                    view,
                    forward: camera.view().transform_vector3(Vec3::Z).extend(0.0),
                    projection: camera.projection(),
                    position: Vec4::new(position.x, position.y, position.z, 1.0),
                }],
            )
            .map_err(|e| e.into())
    }

    /// Updates all GPU camera data from the CPU side camera view and projection
    /// matrix. Position is automatically extracted from the camera's view matrix.
    pub fn update_system() -> BoxedSystem {
        let query = Query::new((camera(), gpu_camera().as_mut()));

        System::builder()
            .with_query(query)
            .with_input::<usize>()
            .build(
                |mut query: QueryBorrow<(Component<Camera>, Mutable<GpuCamera>)>,
                 current_frame: &usize| {
                    query.iter().for_each(|(camera, gpu_camera)| {
                        gpu_camera.update(camera, *current_frame).unwrap();
                    });
                },
            )
            .boxed()
    }

    // Creates gpu side data for all camera which do not already have any.
    pub fn create_gpu_cameras(
        context: &SharedVulkanContext,
        world: &mut World,
        frames_in_flight: usize,
    ) -> Result<()> {
        let mut query = Query::new(entity_ids())
            .with(camera())
            .without(gpu_camera());
        let ids = query.borrow(world).iter().collect_vec();

        ids.iter().try_for_each(|&camera| -> Result<()> {
            let v = GpuCamera::new(context.clone(), frames_in_flight)?;
            world.set(camera, gpu_camera(), v)?;

            Ok(())
        })
    }
}

impl IntoSet for GpuCamera {
    fn set(&self, current_frame: usize) -> DescriptorSet {
        self.sets[current_frame]
    }

    fn sets(&self) -> &[DescriptorSet] {
        &self.sets
    }
}

/// Marker for the main camera
pub struct MainCamera;

#[inline]
pub fn perspective_vk(vertical_fov: f32, aspect_ratio: f32, z_near: f32, z_far: f32) -> Mat4 {
    let t = (vertical_fov / 2.0).tan();
    let sy = 1.0 / t;
    let sx = sy / aspect_ratio;
    let nmf = z_near - z_far;

    Mat4::from_cols(
        Vec4::new(sx, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -sy, 0.0, 0.0),
        Vec4::new(0.0, 0.0, z_far / nmf, -1.0),
        Vec4::new(0.0, 0.0, z_near * z_far / nmf, 0.0),
    )
}

/// Orthographic projection matrix for use with Vulkan.
///
/// This matrix is meant to be used when the source coordinate space is right-handed and y-up
/// (the standard computer graphics coordinate space)and the destination space is right-handed
/// and y-down, with Z (depth) clip extending from 0.0 (close) to 1.0 (far).
#[inline]
pub fn orthographic_vk(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    let rml = right - left;
    let rpl = right + left;
    let tmb = top - bottom;
    let tpb = top + bottom;
    let fmn = far - near;
    Mat4::from_cols(
        Vec4::new(2.0 / rml, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -2.0 / tmb, 0.0, 0.0),
        Vec4::new(0.0, 0.0, -1.0 / fmn, 0.0),
        Vec4::new(-(rpl / rml), -(tpb / tmb), -(near / fmn), 1.0),
    )
}

#[derive(Copy, Default, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Plane {
    p: f32,
    norm: Vec3,
}

impl Plane {
    #[must_use]
    pub fn new(p: f32, norm: Vec3) -> Self {
        Self {
            p,
            norm: norm.normalize(),
        }
    }

    pub fn distance(&self, p: Vec3) -> f32 {
        p.dot(self.norm) + self.p
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Frustum {
    top: Plane,
    bot: Plane,
    left: Plane,
    right: Plane,
    far: Plane,
    near: Plane,
}

impl Frustum {
    pub fn planes(&self) -> [&Plane; 6] {
        [
            &self.top,
            &self.bot,
            &self.left,
            &self.right,
            &self.far,
            &self.near,
        ]
    }

    pub fn ortho(hh: f32, hw: f32, near: f32, far: f32) -> Self {
        let top = Plane::new(hh, Vec3::Y);
        let bot = Plane::new(-hh, -Vec3::Y);
        let right = Plane::new(hw, Vec3::X);
        let left = Plane::new(-hw, -Vec3::X);
        let far = Plane::new(far, Vec3::Z);
        let near = Plane::new(near, Vec3::Z);

        Self {
            top,
            bot,
            left,
            right,
            far,
            near,
        }
    }
    #[must_use]
    pub fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        let hh = (fov / 2.0).tan() * 1.;
        let hw = hh * aspect;

        let nw = vec3(-hw, hh, 1.).normalize();
        let ne = vec3(hw, hh, 1.).normalize();
        let se = vec3(hw, -hh, 1.).normalize();
        let sw = vec3(-hw, -hh, 1.).normalize();

        let top = Plane::new(0., nw.cross(ne));
        let right = Plane::new(0., ne.cross(se));
        let bot = Plane::new(0., se.cross(sw));
        let left = Plane::new(0., sw.cross(nw));
        let far = Plane::new(far, Vec3::Z);
        let near = Plane::new(near, -Vec3::Z);
        // let top = Plane::new(0., ne.cross(nw));
        // let bot = Plane::new(0., sw.cross(se));
        // let right = Plane::new(0., se.cross(ne));
        // let left = Plane::new(0., nw.cross(sw));
        // let far = Plane::new(far, Vec3::Z);

        Self {
            top,
            bot,
            left,
            right,
            far,
            near,
        }
    }

    pub fn visible(&self, p: Vec3, radius: f32) -> bool {
        self.planes().iter().all(|v| v.distance(p) > -radius)
        // self.near.distance(p) > radius
    }
}

impl DrawGizmos for Frustum {
    fn draw_gizmos(&self, gizmos: &mut ivy_core::Gizmos, color: ivy_core::Color) {
        for plane in self.planes() {
            plane.draw_gizmos(gizmos, color)
        }
    }
}

impl DrawGizmos for Plane {
    fn draw_gizmos(&self, gizmos: &mut ivy_core::Gizmos, color: ivy_core::Color) {
        Sphere {
            origin: self.norm * self.p,
            radius: 0.1,
        }
        .draw_gizmos(gizmos, Color::new(0.0, 0.0, 1.0, 1.0));
        Line::new(self.norm * self.p, self.norm, 0.01, 1.0).draw_gizmos(gizmos, color);
    }
}

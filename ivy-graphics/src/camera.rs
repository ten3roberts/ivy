use derive_more::{AsRef, Deref, From, Into};
use ivy_resources::Handle;
use ivy_vulkan::Texture;
use ultraviolet::Mat4;

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
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
/// GPU side camera data
pub struct CameraData {
    pub viewproj: Mat4,
}

impl CameraData {
    pub fn new(viewproj: Mat4) -> Self {
        Self { viewproj }
    }
}

impl Default for CameraData {
    fn default() -> Self {
        Self {
            viewproj: Mat4::identity(),
        }
    }
}

#[derive(AsRef, Deref, Into, From)]
/// The color attachment of a camera.
pub struct ColorAttachment(pub Handle<Texture>);

impl ColorAttachment {
    pub fn new(texture: Handle<Texture>) -> ColorAttachment {
        Self(texture)
    }
}

#[derive(AsRef, Deref, Into, From)]
/// The depth attachment of a camera.
pub struct DepthAttachment(pub Handle<Texture>);

impl DepthAttachment {
    pub fn new(texture: Handle<Texture>) -> DepthAttachment {
        Self(texture)
    }
}
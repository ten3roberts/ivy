use ultraviolet::Mat4;

/// A camera holds a view and projection matrix.
/// Use a system to update view matrix according to position and rotation.
pub struct Camera {
    pub projection: Mat4,
    pub view: Mat4,
}

impl Camera {
    pub fn orthographic(width: f32, height: f32, near: f32, far: f32) -> Self {
        let view = Mat4::identity();

        let hw = width / 2.0;
        let hh = height / 2.0;

        let projection = ultraviolet::projection::orthographic_vk(-hw, hw, -hh, hh, near, far);
        Self { projection, view }
    }

    pub fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> Self {
        let view = Mat4::identity();

        let projection = ultraviolet::projection::perspective_vk(fov, aspect, near, far);

        Self { projection, view }
    }

    /// Calculates the combined view and projection matrix
    pub fn viewproj(&self) -> Mat4 {
        self.projection * self.view
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

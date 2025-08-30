//! Contains common camera components

use flax::{component, Debuggable};
use glam::Mat4;
use ivy_assets::Resource;
use ivy_core::{palette::Srgb, Bundle};
use ivy_editable::Editable;
use serde::{de, Deserialize, Serialize};

component! {
    pub projection_matrix: Mat4,
    pub environment_data: EnvironmentData => [Debuggable],
    pub camera_settings: CameraSettings,
}

#[derive(Debug, Default, Clone, Copy, Resource)]
#[resource(derive = [Editable])]
pub struct CameraBundle {
    camera_settings: CameraSettings,
    environment_data: EnvironmentData,
}

impl CameraBundle {
    pub fn new(camera_settings: CameraSettings, environment_data: EnvironmentData) -> Self {
        Self {
            camera_settings,
            environment_data,
        }
    }
}

impl Bundle for CameraBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        entity
            .set(camera_settings(), self.camera_settings)
            .set(environment_data(), self.environment_data)
            .set_default(projection_matrix());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Editable, serde::Serialize, serde::Deserialize)]
pub struct CameraSettings {
    projection: CameraProjection,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            projection: CameraProjection::perspective(1.2, 0.1, 1000.0),
        }
    }
}

impl CameraSettings {
    pub fn new(projection: CameraProjection) -> Self {
        Self { projection }
    }

    pub fn projection(&self) -> &CameraProjection {
        &self.projection
    }

    pub fn projection_mut(&mut self) -> &mut CameraProjection {
        &mut self.projection
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Editable)]
pub enum CameraProjection {
    Perspective {
        fov_y: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
}

impl CameraProjection {
    pub fn perspective(fov_y: f32, near: f32, far: f32) -> Self {
        Self::Perspective { fov_y, near, far }
    }

    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        Self::Orthographic {
            left,
            right,
            bottom,
            top,
            near,
            far,
        }
    }

    pub fn create_projection_matrix(&self, aspect: f32) -> Mat4 {
        match self {
            CameraProjection::Perspective { fov_y, near, far } => {
                Mat4::perspective_rh(*fov_y, aspect, *near, *far)
            }
            CameraProjection::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => Mat4::orthographic_rh(*left, *right, *bottom, *top, *near, *far),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy, serde::Serialize, serde::Deserialize, Editable)]
pub struct EnvironmentData {
    pub fog_color: Srgb,
    #[editable(range(0.0, 0.01))]
    pub fog_density: f32,
    #[editable(range(0.0, 1.0))]
    pub fog_blend: f32,
}

impl EnvironmentData {
    pub fn new(fog_color: Srgb, fog_density: f32, fog_blend: f32) -> Self {
        Self {
            fog_color,
            fog_density,
            fog_blend,
        }
    }
}

impl Default for EnvironmentData {
    fn default() -> Self {
        Self {
            fog_color: Srgb::new(0.3, 0.3, 0.5),
            fog_density: 0.001,
            fog_blend: 0.0,
        }
    }
}

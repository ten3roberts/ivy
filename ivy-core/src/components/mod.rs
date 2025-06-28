use std::time::Duration;

use flax::{Component, ComponentMut, Debuggable, EntityBuilder, Fetch};
use glam::{Mat4, Quat, Vec2, Vec3};
use ivy_assets::{loadable::ResourceDesc, AssetCache};

use crate::{bundle::Bundle, gizmos::Gizmos, template::BundleDesc, AsyncCommandBuffer, Color};

flax::component! {
    pub position: Vec3 => [Debuggable],
    /// Describes a rotation in 3D space.
    pub rotation: Quat => [ Debuggable ],
    pub scale: Vec3 => [ Debuggable ],

    pub parent_transform: Mat4,

    /// Computed world space transform based on [`position`], [`rotation`], and [`scale`].
    pub world_transform: Mat4 => [ Debuggable ],

    pub size:Vec2 => [ Debuggable ],

    pub is_static: () => [ Debuggable ],

    pub color: Color => [ Debuggable ],

    pub main_camera: () => [ Debuggable ],

    pub gizmos: Gizmos,
    pub asset_cache: AssetCache,
    pub async_commandbuffer: AsyncCommandBuffer,
    pub request_capture_mouse: bool,

    // Set by `ScheduleLayer`
    pub elapsed_time: Duration,
    pub delta_time: Duration,

    pub engine,
}

#[cfg(feature = "serde")]
flax::register_serializable! {
    position,
    rotation,
    scale,
    world_transform,
    main_camera,
    delta_time,
    color,
    is_static
}

#[derive(Fetch, Debug, Clone)]
#[fetch(transforms=[Modified])]
pub struct TransformQuery {
    pub pos: Component<Vec3>,
    pub rotation: Component<Quat>,
    pub scale: Component<Vec3>,
}

impl TransformQuery {
    pub fn new() -> Self {
        Self {
            pos: position(),
            rotation: rotation(),
            scale: scale(),
        }
    }
}

#[derive(Fetch, Debug, Clone)]
pub struct TransformQueryMut {
    pub pos: ComponentMut<Vec3>,
    pub rotation: ComponentMut<Quat>,
    pub scale: ComponentMut<Vec3>,
}

impl TransformQueryMut {
    pub fn new() -> Self {
        Self {
            pos: position().as_mut(),
            rotation: rotation().as_mut(),
            scale: scale().as_mut(),
        }
    }
}

impl Default for TransformQueryMut {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for TransformQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "serde")]
fn one_scale() -> Vec3 {
    Vec3::ONE
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TransformBundle {
    #[cfg_attr(feature = "serde", serde(default))]
    pub position: Vec3,
    #[cfg_attr(feature = "serde", serde(default))]
    pub rotation: Quat,
    #[cfg_attr(feature = "serde", serde(default = "one_scale"))]
    pub scale: Vec3,
}

impl TransformBundle {
    pub fn new(pos: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            position: pos,
            rotation,
            scale,
        }
    }

    /// Set the position
    pub fn with_position(mut self, position: Vec3) -> Self {
        self.position = position;
        self
    }

    /// Set the rotation
    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }

    /// Set the scale
    pub fn with_scale(mut self, scale: Vec3) -> Self {
        self.scale = scale;
        self
    }

    pub fn to_mat4(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

impl Default for TransformBundle {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Bundle for TransformBundle {
    fn mount(&self, entity: &mut EntityBuilder) {
        entity
            .set(position(), self.position)
            .set(rotation(), self.rotation)
            .set(scale(), self.scale)
            .set(
                world_transform(),
                Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position),
            )
            .set(parent_transform(), Default::default());
    }
}

impl ResourceDesc for TransformBundle {
    type Output = TransformBundle;
    type Error = anyhow::Error;

    async fn load(&self, _assets: &AssetCache) -> anyhow::Result<Self::Output> {
        Ok(self.clone())
    }
}

#[typetag::serde]
impl BundleDesc for TransformBundle {}

use std::time::Duration;

use flax::{Component, Debuggable, EntityBuilder, Fetch, Mutable};
use glam::{Mat4, Quat, Vec2, Vec3};

use crate::{gizmos::Gizmos, AsyncCommandBuffer, Bundle, Color};

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
    pub visible: Visible => [ Debuggable ],

    pub color: Color => [ Debuggable ],

    pub main_camera: () => [ Debuggable ],

    pub gizmos: Gizmos,
    pub async_commandbuffer: AsyncCommandBuffer,

    /// Time since last tick
    pub delta_time: Duration,

    pub engine,
}

#[derive(Fetch, Debug, Clone)]
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
    pub pos: Mutable<Vec3>,
    pub rotation: Mutable<Quat>,
    pub scale: Mutable<Vec3>,
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

#[derive(Debug, Clone)]
pub struct TransformBundle {
    pub pos: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl TransformBundle {
    pub fn new(pos: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            pos,
            rotation,
            scale,
        }
    }

    /// Set the position
    pub fn with_position(mut self, position: Vec3) -> Self {
        self.pos = position;
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
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.pos)
    }
}

impl Default for TransformBundle {
    fn default() -> Self {
        Self {
            pos: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Bundle for TransformBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        entity
            .set(position(), self.pos)
            .set(rotation(), self.rotation)
            .set(scale(), self.scale)
            .set(world_transform(), Default::default())
            .set(parent_transform(), Default::default());
    }
}

#[derive(Default, Debug, Clone, Copy)]
/// Marker type for objects that will not move through physics or other means.
/// Objects are assumed to remain in place and not move. Collisions between two
/// static objects will be ignored, useful for level objects which may overlap
/// but not generate collisions
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct Static;

#[derive(Default, Debug, Clone, Copy)]
pub struct Sleeping;

/// Marker type for objects that will not interact with the physics system
/// through collisions despite having colliders.
#[derive(Default, Debug, Clone, Copy)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct Trigger;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
/// Signifies if the entity should be visible or not. Default is true
pub enum Visible {
    /// Entity is fully visible
    Visible,
    /// Entity is explicitly hidden
    Hidden,
    /// Entity is hidden by a parent node
    HiddenInherit,
}

impl Visible {
    /// Returns `true` if the visible is [`Visible`].
    ///
    /// [`Visible`]: Visible::Visible
    #[must_use]
    pub fn is_visible(&self) -> bool {
        matches!(self, Self::Visible)
    }
}

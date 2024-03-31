use std::{borrow::Cow, time::Duration};

use flax::{Component, Debuggable, Fetch};
use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4Swizzles};
use ivy_random::Random;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
mod connections;
mod physics;
pub use connections::*;
pub use physics::*;

flax::component! {
    pub position:Vec3 => [Debuggable],
    /// Describes a rotation in 3D space.
    pub rotation:Quat => [ Debuggable ],
    pub scale:Vec3 => [ Debuggable ],

    /// Calculated scale, rotation, and position transform matrix in world space
    pub transform:Mat4 => [ Debuggable ],

    /// TODO: remove
    pub position2v:Vec2 => [ Debuggable ],

    pub size:Vec3 => [ Debuggable ],

    pub is_static: () => [ Debuggable ],
    pub visible: Visible => [ Debuggable ],

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

impl Default for TransformQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Debug, Clone, Copy)]
/// Marker type for objects that will not move through physics or other means.
/// Objects are assumed to remain in place and not move. Collisions between two
/// static objects will be ignored, useful for level objects which may overlap
/// but not generate collisions
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Static;

#[derive(Default, Debug, Clone, Copy)]
pub struct Sleeping;

/// Marker type for objects that will not interact with the physics system
/// through collisions despite having colliders.
#[derive(Default, Debug, Clone, Copy)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Trigger;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
/// Signifies if the entity should be visible or not. Default is true
pub enum Visible {
    /// Entity is fully visible
    Visible,
    /// Entity is explicitly hidden
    Hidden,
    /// Entity is hidden by a parent node
    HiddenInherit,
}

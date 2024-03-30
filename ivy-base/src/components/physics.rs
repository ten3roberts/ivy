use core::f32;

use flax::Debuggable;
use glam::Vec3;
use ivy_random::Random;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

flax::component! {
    velocity: Vec3 => [ Debuggable ],
    gravity: Vec3 => [ Debuggable ],
    angular_velocity: Vec3 => [ Debuggable ],

    mass: f32 => [ Debuggable ],
    angular_mass: f32 => [ Debuggable ],
    gravity_influence: f32 => [ Debuggable ],
    /// The elasticity of the physics material. A high value means that object is
    /// hard and will bounce back. A value of zero means the energy is absorbed.
    resitution: f32 => [ Debuggable ],
    /// Coefficient of friction
    friction: f32 => [ Debuggable ],
}

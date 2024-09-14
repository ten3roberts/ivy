use core::f32;

use flax::Debuggable;
use glam::Vec3;

// TODO: move to physics crate
flax::component! {
    pub velocity: Vec3 => [ Debuggable ],
    pub gravity: Vec3 => [ Debuggable ],
    pub angular_velocity: Vec3 => [ Debuggable ],

    pub mass: f32 => [ Debuggable ],
    pub inertia_tensor: f32 => [ Debuggable ],
    pub gravity_influence: f32 => [ Debuggable ],
    /// The elasticity of the physics material. A high value means that object is
    /// hard and will bounce back. A value of zero means the energy is absorbed.
    // TODO: move all these to `RigidbodyData` or similar
    pub restitution: f32 => [ Debuggable ],
    /// Coefficient of friction
    pub friction: f32 => [ Debuggable ],

    pub sleeping: () => [ Debuggable ],
    pub is_trigger: () => [ Debuggable ],
}

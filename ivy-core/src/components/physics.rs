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

    pub sleeping: () => [ Debuggable ],
    pub is_trigger: () => [ Debuggable ],
}

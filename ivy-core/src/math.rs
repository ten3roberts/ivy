use glam::{Vec2, Vec3};

pub trait Vec3Ext {
    const FORWARD: Vec3 = Vec3::NEG_Z;
}

impl Vec3Ext for Vec3 {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Axis2D {
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Axis3D {
    X,
    Y,
    Z,
}

impl Axis2D {
    pub fn to_vec2(self) -> Vec2 {
        match self {
            Axis2D::X => Vec2::X,
            Axis2D::Y => Vec2::Y,
        }
    }
}

impl Axis3D {
    pub fn to_vec3(self) -> Vec3 {
        match self {
            Axis3D::X => Vec3::X,
            Axis3D::Y => Vec3::Y,
            Axis3D::Z => Vec3::Z,
        }
    }
}

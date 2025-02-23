use glam::Vec3;

pub trait Vec3Ext {
    const FORWARD: Vec3 = Vec3::NEG_Z;
}

impl Vec3Ext for Vec3 {}

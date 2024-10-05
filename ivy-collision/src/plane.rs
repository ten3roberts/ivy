use glam::Vec3;

pub struct Plane {
    pub distance: f32,
    pub normal: Vec3,
}

impl Plane {
    pub fn new(distance: f32, normal: Vec3) -> Self {
        Self { distance, normal }
    }
}

use ultraviolet::Vec3;

pub trait CollisionPrimitive {
    /// Returns the furtherst vertex in `dir`.
    /// Direction is given in collider/model space.
    fn support(&self, dir: Vec3) -> Vec3;
}

use ultraviolet::Vec3;

pub trait CollisionPrimitive {
    /// Returns the furtherst vertex in `dir`.
    /// Direction is given in collider/model space.
    fn support(&self, dir: Vec3) -> Vec3;
    /// Returns the maximum radius of the primitive. Used for sphere bounding.
    fn max_radius(&self) -> f32;
}

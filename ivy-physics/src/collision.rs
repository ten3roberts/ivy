use ultraviolet::{Mat4, Vec3};

pub const TOLERANCE: f32 = 0.01;

pub trait CollisionPrimitive {
    /// Returns the furtherst vertex in `dir`.
    /// Direction is given in collider/model space.
    fn support(&self, dir: Vec3) -> Vec3;
}

pub struct Cube {
    pub size: f32,
}

impl Cube {
    pub fn new(size: f32) -> Self {
        Self { size }
    }
}

impl CollisionPrimitive for Cube {
    fn support(&self, dir: Vec3) -> Vec3 {
        let x = if dir.x > 0.0 { self.size } else { -self.size };
        let y = if dir.y > 0.0 { self.size } else { -self.size };
        let z = if dir.z > 0.0 { self.size } else { -self.size };

        Vec3::new(x, y, z)
    }
}

pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }
}

impl CollisionPrimitive for Sphere {
    fn support(&self, dir: Vec3) -> Vec3 {
        self.radius * dir
    }
}

/// Generic collider holding any primitive implementing a support function.
pub struct Collider {
    primitive: Box<dyn CollisionPrimitive + Send + Sync>,
}

impl Collider {
    /// Creates a new collider from collision primitive.
    pub fn new<T: 'static + CollisionPrimitive + Send + Sync>(primitive: T) -> Self {
        Self {
            primitive: Box::new(primitive),
        }
    }
}

impl CollisionPrimitive for Collider {
    fn support(&self, dir: Vec3) -> Vec3 {
        self.primitive.support(dir)
    }
}

/// Returns a point on the minkowski difference given from two colliders, their
/// transform, and a direction.
pub fn minkowski_diff<A: CollisionPrimitive, B: CollisionPrimitive>(
    a_transform: &Mat4,
    b_transform: &Mat4,
    a_transform_inv: &Mat4,
    b_transform_inv: &Mat4,
    a_coll: &A,
    b_coll: &B,
    dir: Vec3,
) -> Vec3 {
    assert!((dir.mag() - 1.0).abs() < 0.1);
    let a = a_coll.support(a_transform_inv.transform_vec3(dir).normalized());
    let b = b_coll.support(b_transform_inv.transform_vec3(-dir).normalized());

    let a = a_transform.transform_point3(a);
    let b = b_transform.transform_point3(b);

    a - b
}

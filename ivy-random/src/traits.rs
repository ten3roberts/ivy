use glam::{Vec2, Vec3};
use rand::Rng;

pub trait Random {
    // Generates a random unit vector
    fn rand_unit<R: Rng>(rng: &mut R) -> Self;
    // Generate a vector inside a unit spehre
    fn rand_sphere<R: Rng>(rng: &mut R) -> Self;
    /// Generate a vector withing a sphere with inner and outer radii
    fn rand_constrained_sphere<R: Rng>(rng: &mut R, r1: f32, r2: f32) -> Self;
    // Generates a vector with random components between -1 and 1
    fn rand_uniform<R: Rng>(rng: &mut R) -> Self;
}

impl Random for Vec3 {
    fn rand_unit<R: Rng>(rng: &mut R) -> Self {
        let phi = rng.gen_range(0.0..std::f32::consts::TAU);
        let costheta: f32 = rng.gen_range(-1.0..=1.0);
        let theta = costheta.acos();

        Vec3::new(
            theta.sin() * phi.cos(),
            theta.sin() * phi.sin(),
            theta.cos(),
        )
    }

    fn rand_sphere<R: Rng>(rng: &mut R) -> Self {
        let length = rng.gen_range(0.0..=1.0);
        Self::rand_unit(rng) * length
    }

    fn rand_constrained_sphere<R: Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        let length = rng.gen_range(r1..=r2);
        Self::rand_unit(rng) * length
    }

    fn rand_uniform<R: Rng>(rng: &mut R) -> Self {
        Vec3::new(
            rng.gen_range(-1.0..=1.0),
            rng.gen_range(-1.0..=1.0),
            rng.gen_range(-1.0..=1.0),
        )
    }
}

impl Random for Vec2 {
    fn rand_unit<R: Rng>(rng: &mut R) -> Self {
        let phi = rng.gen_range(0.0..std::f32::consts::TAU);

        Vec2::new(phi.cos(), phi.sin())
    }

    fn rand_sphere<R: Rng>(rng: &mut R) -> Self {
        let length = rng.gen_range(0.0..=1.0);
        Self::rand_unit(rng) * length
    }

    fn rand_constrained_sphere<R: Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        let length = rng.gen_range(r1..=r2);
        Self::rand_unit(rng) * length
    }

    fn rand_uniform<R: Rng>(rng: &mut R) -> Self {
        Vec2::new(rng.gen_range(-1.0..=1.0), rng.gen_range(-1.0..=1.0))
    }
}

impl Random for f32 {
    fn rand_unit<R: Rng>(rng: &mut R) -> Self {
        if rng.gen_bool(0.5) {
            1.0
        } else {
            -1.0
        }
    }

    fn rand_sphere<R: Rng>(rng: &mut R) -> Self {
        rng.gen_range(0.0..=1.0)
    }

    fn rand_constrained_sphere<R: Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        rng.gen_range(r1..=r2)
    }

    fn rand_uniform<R: Rng>(rng: &mut R) -> Self {
        Self::rand_sphere(rng)
    }
}

impl Random for f64 {
    fn rand_unit<R: Rng>(rng: &mut R) -> Self {
        if rng.gen_bool(0.5) {
            1.0
        } else {
            -1.0
        }
    }

    fn rand_sphere<R: Rng>(rng: &mut R) -> Self {
        rng.gen_range(0.0..=1.0)
    }

    fn rand_constrained_sphere<R: Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        rng.gen_range(r1..=r2) as f64
    }

    fn rand_uniform<R: Rng>(rng: &mut R) -> Self {
        Self::rand_sphere(rng)
    }
}

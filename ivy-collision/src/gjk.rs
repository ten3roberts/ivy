use glam::{vec3, Vec3};
use rand::{Rng, SeedableRng};

use crate::{
    util::{minkowski_diff, TOLERANCE},
    Shape, Simplex,
};

/// Performs a gjk intersection test.
/// Returns true if the shapes intersect.
pub fn gjk<A: Shape, B: Shape>(a: &A, b: &B) -> (bool, Simplex) {
    let _span = tracing::debug_span!("gjk").entered();
    // Get first support function in direction of separation
    // let dir = (a_pos - b_pos).normalized();
    let dir = Vec3::X;
    let p1 = minkowski_diff(a, b, dir);

    let mut simplex = Simplex::Point([p1]);

    let mut fallback_directions = [Vec3::X, Vec3::Y, Vec3::Z].into_iter().cycle();

    let mut iteration_count = 0;
    let mut perturberance_rng = rand_pcg::Pcg32::seed_from_u64(42);

    loop {
        // while let Some(dir) = simplex.next_dir() {

        let dir = match simplex.next_dir() {
            crate::SimplexExpansion::Direction(v) => v,
            crate::SimplexExpansion::Degenerate => {
                tracing::debug!("picking new direction");
                fallback_directions.next().unwrap()
            }
            crate::SimplexExpansion::Enveloped => break,
        };

        tracing::debug!(%dir);

        assert!(dir.is_finite(), "{simplex:?}");
        let dir = dir.normalize();

        // Get the next simplex
        let mut p = minkowski_diff(a, b, dir);
        let perturberance_strength = 0.0;
        p.support += vec3(
            perturberance_rng.gen(),
            perturberance_rng.gen(),
            perturberance_rng.gen(),
        ) * perturberance_strength;

        // New point was not past the origin
        // No collision
        if p.support.dot(dir) < 0.0 {
            return (false, simplex);
        }

        simplex.push(p);
        // assert!(simplex.is_unique(), "{simplex:?}");
        iteration_count += 1;
        if iteration_count > 1024 {
            tracing::error!("max gjk iteration");
            return (false, simplex);
        }
    }

    (true, simplex)
}

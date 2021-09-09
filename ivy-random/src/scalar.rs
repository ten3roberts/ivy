use rand::prelude::*;

// Generates a random scalar between 0 and 1
pub fn normalized<R: Rng>(rng: &mut R) -> f32 {
    rng.gen_range(0.0..=1.0)
}

// Generates a random scalar between -1 and 1
pub fn one<R: Rng>(rng: &mut R) -> f32 {
    rng.gen_range(-1.0..=1.0)
}

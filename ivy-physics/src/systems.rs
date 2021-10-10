use hecs::World;
use ivy_core::{Color, Position, Rotation, Scale};
use ivy_graphics::gizmos::{Gizmo, GizmoKind, Gizmos};
use ivy_resources::Resources;
use ultraviolet::{Rotor3, Vec3, Vec4};

use crate::collision::Collider;
use crate::components::{AngularVelocity, TransformMatrix, Velocity};
use crate::gjk;

pub fn integrate_velocity_system(world: &World, dt: f32) {
    world
        .query::<(&mut Position, &Velocity)>()
        .iter()
        .for_each(|(_, (pos, vel))| *pos += Position(vel.0 * dt));
}

pub fn integrate_angular_velocity_system(world: &World, dt: f32) {
    world
        .query::<(&mut Rotation, &AngularVelocity)>()
        .into_iter()
        .for_each(|(_, (rot, ang))| {
            let (x, y, z) = (ang.x, ang.y, ang.z);
            *rot = Rotation(**rot * Rotor3::from_euler_angles(x * dt, y * dt, z * dt));
        });
}

pub fn gravity_system(world: &World, dt: f32) {
    world
        .query::<&mut Velocity>()
        .iter()
        .for_each(|(_, vel)| vel.y -= 1.0 * dt)
}

pub fn wrap_around_system(world: &World) {
    world.query::<&mut Position>().iter().for_each(|(_, pos)| {
        if pos.y < -100.0 {
            pos.y = 100.0
        }
    });
}

pub fn collision_system(world: &World, resources: &Resources) -> ivy_resources::Result<()> {
    world
        .query::<(&Position, &Rotation, &Scale, &Collider, &mut Color)>()
        .iter()
        .try_for_each(|(e1, (pos, rot, scale, a, color))| -> Result<_, _> {
            let a_transform = TransformMatrix::new(*pos, *rot, *scale);
            world
                .query::<(&Position, &Rotation, &Scale, &Collider)>()
                .iter()
                .try_for_each(|(e2, (pos, rot, scale, b))| -> Result<_, _> {
                    let b_transform = TransformMatrix::new(*pos, *rot, *scale);
                    if e1 == e2 {
                        return Ok(());
                    }

                    let (intersect, simplex) = gjk::intersection(a_transform, b_transform, a, b);
                    // assert!(matches!(simplex, Simplex::Tetrahedron(a, b, c, d)));

                    let mut gizmos = resources.get_default_mut::<Gizmos>()?;

                    gizmos.push(Gizmo::new(
                        Vec3::zero(),
                        Vec4::new(0.0, 0.0, 1.0, 1.0),
                        GizmoKind::Sphere(0.1),
                    ));

                    for point in simplex.points() {
                        gizmos.push(Gizmo::new(
                            *point,
                            Vec4::new(1.0, 0.0, 0.0, 1.0),
                            GizmoKind::Sphere(0.1),
                        ));
                    }

                    if intersect {
                        *color = Color::new(1.0, 0.0, 0.0, 1.0);
                    } else {
                        *color = Color::new(1.0, 1.0, 1.0, 1.0);
                    }

                    Ok(())
                })
        })
}

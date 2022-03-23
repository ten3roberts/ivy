use hecs::{Component, Entity, World};
use ivy_engine::{AngularVelocity, Input, InputVector, Rotation, Velocity};
use ivy_physics::Effector;

pub struct WithTime<T> {
    func: Box<dyn Fn(Entity, &mut T, f32, f32) + Send + Sync>,
    elapsed: f32,
}

impl<T> WithTime<T>
where
    T: Component,
{
    pub fn new(func: Box<dyn Fn(Entity, &mut T, f32, f32) + Send + Sync>) -> Self {
        Self { func, elapsed: 0.0 }
    }

    pub fn update(world: &mut World, dt: f32) {
        world
            .query::<(&mut Self, &mut T)>()
            .iter()
            .for_each(|(e, (s, val))| {
                s.elapsed += dt;
                (s.func)(e, val, s.elapsed, dt);
            });
    }
}

pub struct Mover {
    pub translate: InputVector,
    pub rotate: InputVector,
    pub local: bool,
    pub speed: f32,
}

impl Mover {
    pub fn new(translate: InputVector, rotate: InputVector, speed: f32, local: bool) -> Self {
        Self {
            local,
            translate,
            rotate,
            speed,
        }
    }
}

pub fn move_system(world: &mut World, input: &Input) {
    world
        .query::<(
            &Mover,
            &mut Velocity,
            &mut Effector,
            &mut AngularVelocity,
            &Rotation,
        )>()
        .iter()
        .for_each(|(_, (m, v, e, a, r))| {
            let movement = m.translate.get(&input);
            if m.local {
                *v = Velocity(**r * movement) * m.speed;
            } else {
                *v = Velocity(movement) * m.speed;
            }

            let ang = m.rotate.get(&input);
            *a = ang.into();
            e.wake()
        })
}

use flax::{
    component::{self, ComponentValue},
    BoxedSystem, Component, Entity, FetchExt, Query, System, World,
};
use image::imageops::rotate90_in;
use ivy_base::{angular_velocity, engine, position, rotation, velocity};
use ivy_engine::{InputState, InputVector};
use ivy_input::components::input_state;
use ivy_physics::{components::effector, Effector};

flax::component! {
    pub mover: Mover,
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

pub fn move_system() -> BoxedSystem {
    System::builder()
        .with_name("move_system")
        .with_query(Query::new((
            input_state().source(engine()),
            mover(),
            velocity().as_mut(),
            effector().as_mut(),
            angular_velocity().as_mut(),
            rotation(),
            position(),
        )))
        .for_each(
            |(input_state, mover, velocity, effector, ang_vel, rotation, position)| {
                let movement = mover.translate.get(&input_state);
                // tracing::info!(%movement, %velocity, %position, %mover.speed, "move system");
                if mover.local {
                    *velocity = *rotation * movement * mover.speed;
                } else {
                    *velocity = movement * mover.speed;
                }

                let ang = mover.rotate.get(&input_state);
                *ang_vel = ang;
                effector.wake()
            },
        )
        .boxed()
}

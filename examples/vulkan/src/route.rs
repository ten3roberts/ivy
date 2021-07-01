use hecs::World;
use ivy_ui::constraints::RelativeOffset;

#[derive(Debug)]
pub struct Route2D {
    points: Vec<RelativeOffset>,
    pub speed: f32,
    pub current: usize,
}

impl Route2D {
    pub fn new(points: Vec<RelativeOffset>, speed: f32) -> Self {
        Self {
            points,
            speed,
            current: 0,
        }
    }
}

pub fn update_routes(world: &World, dt: f32) {
    world
        .query::<(&mut RelativeOffset, &mut Route2D)>()
        .iter()
        .for_each(|(_, (pos, route))| {
            let current_point = route.points[route.current];
            let relative = current_point - *pos;

            *pos += (relative.normalized() * route.speed * dt).into();

            if relative.mag_sq() < dt * dt * route.speed * route.speed {
                route.current = (route.current + 1) % route.points.len();
            }
        });
}

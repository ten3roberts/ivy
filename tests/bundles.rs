use flax::{components::name, Entity};
use glam::vec3;
use ivy_base::{restitution, EntityBuilderExt};
use ivy_engine::{App, RbBundle};

#[test]
fn bundles() {
    let mut app = App::builder().build();

    let world = app.world_mut();

    let entity = Entity::builder()
        .mount(RbBundle {
            vel: vec3(1.0, 0.0, 0.0),
            mass: 5.0,
            ang_mass: 2.0,
            ..Default::default()
        })
        .set(name(), "My Entity".into())
        .spawn(world);

    // Get the `Resitution` component
    assert_eq!(*world.get(entity, restitution()).unwrap(), 0.0,);
}

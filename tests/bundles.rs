use flax::{components::name, Entity};
use glam::Vec3;
use ivy_core::EntityBuilderExt;
use ivy_engine::{position, App, TransformBundle};

#[test]
fn bundles() {
    let mut app = App::builder().build();

    let world = app.world_mut();

    let entity = Entity::builder()
        .mount(TransformBundle::default())
        .set(name(), "My Entity".into())
        .spawn(world);

    // Get the `Resitution` component
    assert_eq!(*world.get(entity, position()).unwrap(), Vec3::ZERO);
}

use hecs::EntityBuilder;
use ivy::{AngularMass, App, Mass, Name, RbBundle, Resitution, Velocity};
use ivy_base::BuilderExt;

#[test]
fn bundles() {
    let mut app = App::builder().build();

    let world = app.world_mut();

    let entity = EntityBuilder::new()
        .add_bundle(RbBundle {
            vel: Velocity::new(1.0, 0.0, 0.0),
            mass: Mass(5.0),
            ang_mass: AngularMass(2.0),
            ..Default::default()
        })
        .add(Name::new("My Entity"))
        .spawn(world);

    // Get the `Resitution` component
    assert_eq!(
        *world.get::<Resitution>(entity).unwrap(),
        Resitution::default()
    );
}

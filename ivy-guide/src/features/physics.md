# Physics & Simulation
Learn how to integrate physics into your Ivy applications for realistic object interactions and simulations.

## Setting Up Physics
Add the physics plugin to enable physics simulation:

```rust
use ivy_physics::PhysicsPlugin;

app.with_plugin(PhysicsPlugin::new().with_gravity(Vec3::new(0.0, -9.81, 0.0)));

## Creating Physics Objects
Spawn entities with physics components:

```rust
// Rigid body with collider
world.spawn()
    .set_bundle(TransformBundle {
        position: Vec3::new(0.0, 5.0, 0.0),
        ..Default::default()
    })
    .set_bundle(RigidBodyBundle::dynamic().with_mass(1.0))
    .set_bundle(ColliderBundle::new(rapier3d::prelude::SharedShape::ball(1.0)));
```

## Applying Forces and Impulses
Control objects dynamically in your systems:

```rust
fn apply_forces(
    mut velocities: Query<&mut Velocity>,
    time: Res<Time>,
) {
    for mut vel in &mut velocities {
        // Apply continuous force (like gravity)
        vel.0 += Vec3::Y * -9.81 * time.delta_seconds();

        // Apply impulse (like jump)
        vel.0 += Vec3::Y * 10.0;
    }
}
```

## Handling Collisions
Respond to collision events:

```rust
fn collision_handler(
    mut events: EventReader<EntityCollisionEvent>,
) {
    for event in events.iter() {
        println!("Collision between {:?} and {:?}",
                 event.entity1, event.entity2);

        // Play sound, apply damage, etc.
    }
}
```

## Joints and Constraints
Connect objects with joints:

```rust
// Fixed joint (weld objects together)
world.spawn()
    .set(impulse_joint(body2), rapier3d::prelude::GenericJoint::Fixed(rapier3d::prelude::FixedJoint::new()));

// Hinge joint (doors, wheels)
world.spawn()
    .set(impulse_joint(door), rapier3d::prelude::GenericJoint::Revolute(rapier3d::prelude::RevoluteJoint::new(Vec3::Y)));
```

## Ray Casting
Query the physics world for intersections:

```rust
fn raycast_system(
    physics: Res<PhysicsState>,
    camera: Query<(&Transform, &Camera)>,
) {
    let (transform, camera) = camera.single();

    // Cast ray from camera
    let ray = Ray3d::new(transform.translation, transform.forward());
    let max_distance = 100.0;

    if let Some(hit) = physics.cast_ray(&ray, max_distance, true) {
        // Hit something at hit.point with normal hit.normal
        // Entity ID is available in hit.entity
    }
}
```

## Character Controllers
Use pre-built character movement:

```rust
use ivy_game::character_controller::{CharacterController, CharacterControllerBundle};

world.spawn()
    .set_bundle(CharacterControllerBundle {
        controller: CharacterController {
            speed: 5.0,
            jump_force: 10.0,
            ..Default::default()
        },
        ..Default::default()
    });
```

## Performance Optimization
- Use simple colliders when possible (spheres/cubes over meshes)
- Disable physics on static objects
- Use physics layers to control collision groups
- Profile with physics debug visualization

# Entity Component System
Ivy's game logic is centered around the Entity Component System (ECS) design pattern using Flax.

## ECS Fundamentals
In ECS:
- **Entities** are simple IDs that represent game objects
- **Components** are data attached to entities (position, velocity, health, etc.)
- **Systems** operate on component queries to implement game logic

This data-oriented approach enables efficient, cache-friendly code and parallel execution.

## Flax ECS
Ivy uses [Flax](https://github.com/ten3roberts/flax) for its ECS implementation, providing:

- Fast component storage and querying
- Automatic system scheduling and parallelization
- Type-safe component access
- Entity relationships and hierarchies

## Components
Components are defined using Flax's `component!` macro:

```rust
use flax::component;

#[derive(Clone, Debug)]
struct Position(glam::Vec3);

#[derive(Clone, Debug)]
struct Velocity(glam::Vec3);

component! {
    position: Position,
    velocity: Velocity,
}
```

## Entities
Entities are created and managed through the World:

```rust
use flax::World;

let mut world = World::new();

// Create an entity with components
let entity = world.spawn()
    .set(position(), Position(glam::Vec3::ZERO))
    .set(velocity(), Velocity(glam::Vec3::X))
    .id();
```

## Systems
Systems query and operate on entities with specific components:

```rust
use flax::{system, Query};

// Define a system that updates positions based on velocity

#[system]
fn update_positions(
    mut positions: Query<(&mut Position, &Velocity)>,
    dt: &f32,
) {
    for (pos, vel) in &mut positions {
        pos.0 += vel.0 * *dt;
    }
}
```

## Queries
Queries efficiently find entities with specific component combinations:

```rust
// Query entities with both position and velocity
let query = Query::new((position(), velocity()));

// Iterate over matching entities
for (entity, (pos, vel)) in &query {
    // Do something with position and velocity
}
```

## Bundles
Bundles group related components for easy entity creation:

```rust
use ivy_core::Bundle;

#[derive(Bundle)]
struct PhysicsBundle {
    position: Position,
    velocity: Velocity,
}

let entity = world.spawn_bundle(PhysicsBundle {
    position: Position(glam::Vec3::ZERO),
    velocity: Velocity(glam::Vec3::X),
});
```

ECS provides the foundation for scalable, performant game logic in Ivy.

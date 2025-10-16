# ivy-physics
Physics simulation using Rapier3D.

## Key Components
### PhysicsState
Wraps Rapier physics world.

### Components
- `rigidbody_builder`, `collider_builder`: Physics body construction
- `rb_handle`, `collider_handle`: Physics object handles
- `velocity`, `gravity`, `mass`: Physical properties
- `effector`: Custom force application

### Effector Trait
For applying forces/torques to physics objects.

### EntityCollisionEvent
Collision events with entity information.

## Modules
- `bundles`: Physics bundles for entities
- `components`: Physics components
- `effector`: Force/torque effectors
- `plugin`: Physics plugin
- `state`: Physics world state
- `systems`: Physics update systems
- `shapes`: Collision shapes
- `util`: Physics utilities

# ivy-core
The base crate providing core types, traits, and the App framework.

## Key Components
### App
The heart of Ivy programs, manages layers, ECS world, asset cache, and event system. Contains `World` (ECS), `AssetCache`, `EventRegistry`, and `DynamicStore`.

### Layer Trait
Abstraction for logic layers (e.g., graphics, game logic). Implement `register` to set up systems and events.

### Plugin Trait
For ECS-centered modular logic, adding systems and schedules to the World. Enables multithreaded execution.

### EngineLayer
Default layer handling async command buffers and gizmos.

### Gizmos
Debug visualization system for temporary objects. Includes `GizmosSection`, `DrawGizmos` trait, and gizmo types like `SphereGizmo`, `LineGizmo`, `CuboidGizmo`.

### TransformBundle
Combines position, rotation, scale for 3D transformations. Implements `Bundle`.

### Bundle Trait
For mounting sets of components to entities via `EntityBuilder`.

### AsyncCommandBuffer
For deferred ECS operations.

### Template
Composition of Bundles for entity construction; allows stored, serializable entity descriptions.

## Modules
- `app`: Builder, driver, event handling
- `bundle`: Bundle traits and implementations
- `components`: Core ECS components (position, rotation, scale, etc.)
- `extensions`: Utility extensions
- `gizmos`: Debug visualization
- `layer`: Layer traits and events
- `plugin`: Plugin system
- `subscribers`: Event subscribers
- `systems`: ECS systems
- `template`: Entity templates
- `transforms`: 3D transformations
- `updatable`: Update patterns
- `update_layer`: Update scheduling

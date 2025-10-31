# Architecture
Ivy Engine is a Rust-based application and game framework designed for building graphics-intensive applications, including games. It provides a layered architecture for organizing logic, an ECS (Entity Component System) via Flax, asset management, physics simulation with Rapier3D, and rendering via WebGPU (wgpu).

The engine is organized into multiple crates within a workspace, allowing modular development and optional features.

## App and Layers
At the heart of Ivy is the `App` struct, which orchestrates the entire application lifecycle. The `App` manages:

- **ECS World**: A Flax-based Entity Component System storing all entities and their components.
- **Asset Cache**: Asynchronous asset loading and caching system with hot reloading support.
- **Event Registry**: A broadcasting system for inter-layer communication.
- **Layers**: Modular units of logic that can be stacked and configured.

Layers implement the `Layer` trait, providing methods for initialization (`register`), updates, and event handling. The layered design supports composable, non-interfering systems. For example:

- A rendering layer handles GPU operations.
- A physics layer simulates rigid body dynamics.
- An input layer processes user interactions.
- Custom game layers implement specific logic.

Layers communicate through shared resources:
- **ECS World**: Entities and components are accessible across layers.
- **Asset Cache**: Shared assets like textures and models.

## Plugins and World Logic
Plugins extend the ECS World with modular logic using the `Plugin` trait. Unlike Layers, which operate above the World, plugins integrate directly into the World's systems and scheduled logic. They allow you to add ECS systems for game and entity logic, and enable automatic multithreading for parallel execution.

Plugins are registered during App setup and can depend on other plugins for dependency-ordered execution.

This allows decoupled game systems that can be mixed, matched, and conditionally enabled/disabled based on application needs.

Plugins intentionally cannot access each other; instead, they communicate through the shared ECS data and world resources.

## ECS and Components
Ivy uses an Entity Component System (ECS) for game logic. Entities are simple IDs, components are data attached to entities, and systems operate on component queries.

**Core Components**:
- **Transforms**: `position`, `rotation`, `scale`, `world_transform` for 3D positioning.
- **Rendering**: `mesh`, `material`, `camera`, `light` for graphics.
- **Physics**: `rigidbody_builder`, `collider_builder`, `velocity`, `mass` for simulation.
- **Custom**: User-defined components for game-specific data.

Components are defined using Flax's `component!` macro and can be bundled together using the `Bundle` trait for easy entity creation.

## Asset Management
Assets are loaded asynchronously to avoid blocking the main thread. The `AssetCache` handles:

- **Caching**: Loaded assets are stored and reused.
- **Hot Reloading**: Assets reload automatically on file changes during development.
- **Types**: `Asset<T>` for shared resources, `Resource` for owned data.

Supported formats include GLTF models, images (PNG, JPEG, HDR), and custom types via the extensible asset system.

## Rendering Pipeline
Rendering is handled via WebGPU through the WGPU backend:

- **Render Graph**: Defines multi-pass rendering pipelines with dependencies.
- **Shaders**: WGSL shaders for materials, lighting, and effects.
- **Post-Processing**: Effects like bloom and HDR tonemapping applied after main rendering.
- **Gizmos**: Debug visualization primitives for development.

The pipeline supports PBR materials, shadows, and advanced lighting.

## Physics Simulation
Physics is powered by Rapier3D, integrated seamlessly with ECS:

- **Rigid Bodies**: Dynamic objects with mass and velocity.
- **Colliders**: Shapes for collision detection (spheres, boxes, meshes).
- **Joints**: Constraints between bodies.
- **Effectors**: Custom forces and torques.

Physics updates run in sync with the game loop, with collision events fed back into the ECS.

## Input Handling
Input is processed through a flexible system:

- **Events**: Winit events are converted to Ivy `InputEvent`s.
- **Actions**: Bind inputs to stimuli (bool, f32, Vec2) that update components or trigger callbacks.
- **Bindings**: Composable mappings for keyboard, mouse, and gamepad.

This allows for customizable control schemes without hardcoding.

## UI Integration
UI is built on the Violet retained-mode GUI library:

- **Widgets**: Configurable components for layout and interaction.
- **Integration**: UI renders as an overlay on 3D scenes.
- **Editor**: In-engine editing tools use the same UI system.

## Templates and Entity Construction
Templates define and instantiate entities:

- **Composition**: A set of bundles describing the entity.
- **Serialization**: Templates support serialization and deserialization for loading assets like textures and models.
- **Editing**: Templates are editable in the editor for visual entity creation.
- **Instantiation**: Entity spawning from stored templates for asset-based game design.

Templates combine programmatic and data-driven entity creation, supporting visual editing.

## Signals
Signals allow entities to declare response capabilities for events, acting like open sockets that other code can invoke on the entity. Entities wanting to receive events (e.g., "onCollision", "unequip", "mouseButtonChanged") declare signals, enabling decoupled invocation without tight coupling. This supports reactive behaviors, such as collision handling or character tool switching.

# Ivy Engine

[![Rust](https://img.shields.io/badge/rust-orange?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![WebGPU](https://img.shields.io/badge/WebGPU-blue?style=for-the-badge&logo=webgpu)](https://gpuweb.github.io/gpuweb/)
[![ECS](https://img.shields.io/badge/ECS-Flax-green?style=for-the-badge)](https://github.com/ten3roberts/flax)
[![Violet](https://img.shields.io/badge/GUI-Violet-purple?style=for-the-badge)](https://github.com/ten3roberts/violet)

[![Docs](https://img.shields.io/badge/API%20Docs-lib.rs?style=for-the-badge)](https://lib.rs/ivy)

A modular, ECS-driven game engine written in Rust, designed for building high-performance 3D applications and games. Ivy provides a layered architecture, advanced rendering with WebGPU, physics simulation, asset management, and an integrated editor.

## Table of Contents

- [Overview](#overview)
- [Key Features](#key-features)
- [Working Principles](#working-principles)
- [Serialization](#serialization)
- [Getting Started](#getting-started)
- [Examples](#examples)
- [Documentation](#documentation)
- [Gallery](#gallery)
- [Contributing](#contributing)
- [License](#license)

## Overview

Ivy Engine is built around an Entity Component System (ECS) using Flax, enabling efficient data-oriented programming. It features a modular crate structure, allowing developers to pick and choose components for their projects. The engine supports WebGPU-based rendering, physics with Rapier3D, async asset loading, and more.

**Systems communicate not by direct action, but by sharing data**

Ivy allows Rust applications for games, simulations, and interactive 3D apps with a decoupled, modular architecture that avoids spaghetti code.

## Key Features

### Core Systems
- **ECS Architecture**: Entity Component System via Flax for efficient game logic.
- **Layered Design**: Modular layers for organizing logic (rendering, physics, input, etc.).
- **Async Asset Management**: Non-blocking asset loading with caching and hot reloading.
- **Event System**: Top-down propagated system events (e.g., input, window resize) from the application driver to layers using an observer pattern.
- **Signals**: Entity-to-entity gameplay events for cross-entity communication.

### Rendering & Graphics
- **WebGPU Backend**: Modern GPU API support via WGPU.
- **PBR Rendering**: Physically Based Rendering with materials, lights, and shadows.
- **Render Graph**: Abstractions for fine-tuned render pipelines.
- **Post-Processing**: Effects including bloom, depth of field, HDR tonemapping, color grading, and custom shaders.
- **Gizmos**: Debug visualization for development.

### Physics & Simulation
- **Integrated Physics**: Full 3D physics with Rapier3D.
- **Collision Detection**: Rigid bodies, colliders, joints, and effectors.
- **Force Application**: Custom effectors for dynamic simulations.

### Input & Interaction
- **Flexible Input System**: Composable vector generation from keyboard, mouse, and controllers.
- **Action Binding**: Map inputs to component updates or callbacks.

### User Interface
- **UI System**: Built on Violet GUI library with configurable widgets and positioning.
- **Editor Integration**: In-engine editor with property editing and gizmos.

### Asset Pipeline
- **GLTF Support**: Load 3D models, animations, and scenes.
- **Image Loading**: PNG, JPEG, HDR, and more.
- **Custom Assets**: Extensible asset types with serialization.

### Utilities
- **Profiling**: Optional Puffin integration for performance analysis.
- **Random Generation**: Utilities for procedural content.
- **Reflection**: Basic reflection traits for editor and serialization.

## Working Principles

### Core Architecture: App and Layers

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

### Plugins and World Logic
Plugins extend the ECS World with modular logic using the `Plugin` trait. Unlike Layers, which operate above the World,
plugins integrate directly into the World's systems and scheduled logic. They allow you to add ECS systems for game and
entity logic, and enable automatic multithreading for parallel execution.

Plugins are registered during App setup and can depend on other plugins for dependency-ordered execution.

This allows decoupled game systems that can be mixed, matched, and conditionally enabled/disabled based on application needs.

Plugins intentionally cannot access each other; instead, they communicate through the shared ECS data and world
resources.

### ECS and Components

Ivy uses an Entity Component System (ECS) for game logic. Entities are simple IDs, components are data attached to entities, and systems operate on component queries.

**Core Components**:
- **Transforms**: `position`, `rotation`, `scale`, `world_transform` for 3D positioning.
- **Rendering**: `mesh`, `material`, `camera`, `light` for graphics.
- **Physics**: `rigidbody_builder`, `collider_builder`, `velocity`, `mass` for simulation.
- **Custom**: User-defined components for game-specific data.

Components are defined using Flax's `component!` macro and can be bundled together using the `Bundle` trait for easy entity creation.

### Asset Management

Assets are loaded asynchronously to avoid blocking the main thread. The `AssetCache` handles:
- **Caching**: Loaded assets are stored and reused.
- **Hot Reloading**: Assets reload automatically on file changes during development.
- **Types**: `Asset<T>` for shared resources, `Resource` for owned data.

Supported formats include GLTF models, images (PNG, JPEG, HDR), and custom types via the extensible asset system.

### Rendering Pipeline

Rendering is handled via WebGPU through the WGPU backend:
- **Render Graph**: Defines multi-pass rendering pipelines with dependencies.
- **Shaders**: WGSL shaders for materials, lighting, and effects.
- **Post-Processing**: Effects like bloom and HDR tonemapping applied after main rendering.
- **Gizmos**: Debug visualization primitives for development.

The pipeline supports PBR materials, shadows, and advanced lighting.

### Physics Simulation

Physics is powered by Rapier3D, integrated seamlessly with ECS:
- **Rigid Bodies**: Dynamic objects with mass and velocity.
- **Colliders**: Shapes for collision detection (spheres, boxes, meshes).
- **Joints**: Constraints between bodies.
- **Effectors**: Custom forces and torques.

Physics updates run in sync with the game loop, with collision events fed back into the ECS.

### Input Handling

Input is processed through a flexible system:
- **Events**: Winit events are converted to Ivy `InputEvent`s.
- **Actions**: Bind inputs to stimuli (bool, f32, Vec2) that update components or trigger callbacks.
- **Bindings**: Composable mappings for keyboard, mouse, and gamepad.

This allows for customizable control schemes without hardcoding.

### UI Integration

UI is built on the Violet retained-mode GUI library:
- **Widgets**: Configurable components for layout and interaction.
- **Integration**: UI renders as an overlay on 3D scenes.
- **Editor**: In-engine editing tools use the same UI system.

### Templates and Entity Construction

Templates define and instantiate entities:
- **Composition**: A set of bundles describing the entity.
- **Serialization**: Templates support serialization and deserialization for loading assets like textures and models.
- **Editing**: Templates are editable in the editor for visual entity creation.
- **Instantiation**: Entity spawning from stored templates for asset-based game design.

Templates combine programmatic and data-driven entity creation, supporting visual editing.

### Signals

Signals allow entities to declare response capabilities for events, acting like open sockets that other code can invoke on the entity. Entities wanting to receive events (e.g., "onCollision", "unequip", "mouseButtonChanged") declare signals, enabling decoupled invocation without tight coupling. This supports reactive behaviors, such as collision handling or character tool switching.

## Serialization

Ivy supports scene serialization and save/loading for persistent worlds and editors:

- **Scene Serialization**: Scenes serialize to JSON, Bincode, and RON, capturing entity hierarchies via Templates.
- **Template-Based**: Each entity references a `Template` (via `template_path`), with component data serialized separately.
- **Asset Integration**: Serialized scenes load asynchronously, resolving asset dependencies.
- **Editor Support**: The integrated editor includes save/load dialogs for `.ivsc` scene files.
- **API**: Use `SceneSerializer` for programmatic serialization; `serialize_json` for JSON saving, `load_scene` for loading from any supported format.

This supports rapid iteration, scene sharing, and data-driven content creation without custom code.

### Editor and Tools

Ivy includes an integrated editor for rapid development:
- **Gizmos**: Visual debugging tools.
- **Property Editing**: Modify component values in real-time.
- **Template Editing**: Create and edit entity templates visually.
- **Scene Management**: Load, edit, and save scenes.

The editor is built using the same ECS and UI systems as user applications.

## Getting Started

### Prerequisites
- Rust 1.70 or later
- A WebGPU-compatible GPU (most modern GPUs)

### Installation
Add Ivy to your `Cargo.toml`:

```toml
[dependencies]
ivy-engine = "0.10"
```

For specific crates:
```toml
ivy-core = "0.10"
ivy-wgpu = "0.10"
ivy-physics = "0.10"
# etc.
```

### Quick Start
```rust
use ivy_core::{App, EngineLayer, transforms::TransformUpdatePlugin, update_layer::{FixedTimeStep, PluginLayer}};
use ivy_game::fly_camera::FlyCameraPlugin;
use ivy_input::layer::InputLayer;
use ivy_physics::PhysicsPlugin;
use ivy_postprocessing::preconfigured::{SurfacePbrPipelineDesc, SurfacePbrRenderer};
use ivy_wgpu::{driver::WinitDriver, layer::GraphicsLayer};
use winit::window::WindowAttributes;

fn main() -> anyhow::Result<()> {
    App::builder()
        .with_driver(WinitDriver::new(WindowAttributes::default()))
        .with_layer(EngineLayer::new())
        .with_layer(GraphicsLayer::new(|world, assets, store, gpu, surface| {
            Ok(SurfacePbrRenderer::new(
                world, assets, store, gpu, surface,
                SurfacePbrPipelineDesc::default(),
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(PluginLayer::new(FixedTimeStep::new(0.02))
            .with_plugin(FlyCameraPlugin)
            .with_plugin(PhysicsPlugin::new().with_gravity(-glam::Vec3::Y * 9.81))
            .with_plugin(TransformUpdatePlugin))
        .run()
}
```

## Examples

Check out the `examples/` directory for:
- Basic rendering setup
- Physics simulations
- Input handling
- UI integration
- Full game examples

Run an example:
```bash
cargo run --example basic
```

## Documentation

- **[User Guide](https://ten3roberts.github.io/ivy)**: Comprehensive guide to using Ivy.
- **[API Docs](https://lib.rs/ivy)**: Generated Rust documentation.
- **[Structure.md](./Structure.md)**: Detailed engine architecture and crate breakdown.

## Gallery

### Basic PBR Scene
![PBR example](https://github.com/user-attachments/assets/a83689d0-42fb-4002-804c-921b6702dc8f)

### Emissive Materials
![Emissive Materials](https://github.com/user-attachments/assets/8e640d28-345c-44f7-b607-94febb1682fc)

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

- Report issues on [GitHub Issues](https://github.com/ten3roberts/ivy-engine/issues)
- Submit PRs for bug fixes, features, or documentation improvements

## License

Licensed under MIT OR Apache-2.0. See [LICENSE](./LICENSE) for details.

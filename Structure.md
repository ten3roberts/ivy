# Ivy Engine Structure and Functionality

## Overview

Ivy Engine is a Rust-based application and game framework designed for building graphics-intensive applications, including games. It provides a layered architecture for organizing logic, an ECS (Entity Component System) via Flax, asset management, physics simulation with Rapier3D, and rendering via WebGPU (wgpu).

The engine is organized into multiple crates within a workspace, allowing modular development and optional features.

## Crate Structure

### Core Crates

#### ivy-core
- **Purpose**: Base crate providing core types, traits, and the App framework.
- **Key Components**:
  - `App`: Heart of Ivy programs, manages layers, ECS world, asset cache, and event system. Contains `World` (ECS), `AssetCache`, `EventRegistry`, and `DynamicStore`.
  - `Layer` trait: Abstraction for logic layers (e.g., graphics, game logic). Implement `register` to set up systems and events.
  - `LayerDyn`: Dynamic dispatch for layers, with `register_dyn`.
  - `EngineLayer`: Default layer handling async command buffers and gizmos.
  - `Gizmos`: Debug visualization system for temporary objects. Includes `GizmosSection`, `DrawGizmos` trait.
  - Gizmo types: `SphereGizmo`, `LineGizmo`, `CuboidGizmo`, etc., implementing `DrawGizmos`.
  - Components: `position`, `rotation`, `scale`, `world_transform`, `color`, `main_camera`, `gizmos`, `asset_cache`, `async_commandbuffer`, `elapsed_time`, `delta_time`, `engine`.
  - `TransformBundle`: Combines position, rotation, scale for 3D transformations. Implements `Bundle`.
  - `Bundle` trait: For mounting sets of components to entities via `EntityBuilder`.
  - `AsyncCommandBuffer`: For deferred ECS operations.
  - Events: `TickEvent`, `PostInitEvent`, etc.
- **Modules**: app (builder, driver, event), bundle, components, extensions, gizmos (traits, transforms), layer (events), math, subscribers, systems, template, transforms, updatable, update_layer.
- **Relations**: App owns World and AssetCache; Layers register into World; Gizmos are drawn via DrawGizmos; Bundles mount to EntityBuilder; Components are ECS primitives.

#### ivy-assets
- **Purpose**: Sync and async asset system with caching.
- **Key Components**:
  - `AssetCache`: Central store for assets, manages loading, caching, and services. Contains pending keys, keys (DashMap), cells, services (RwLock), timelines.
  - `Asset<T>`: Shared handle to loaded assets, wraps `Arc<T>` with `AssetId` for comparison.
  - `WeakAsset<T>`: Weak reference to assets.
  - `AssetLoadFuture<T>`: Future for async asset loading, can be polled or awaited.
  - `Resource` trait: For types that can be loaded from descriptions, with associated `Desc`.
  - `Loadable` trait: Generic loading mechanism, converts description to resource.
  - `LoadFromPath` trait: For loading from file paths, with extensions and resource name.
  - `AssetDesc` trait: Keys for loading assets synchronously.
  - `AsyncAssetKey` trait: Keys for async loading.
  - `Service` trait: Registered with AssetCache for loading (e.g., `FileSystemMapService` for file access).
  - `AssetPath<T>`: Path wrapper for typed asset loading.
  - Hot reloading via `FileReloadService`.
- **Modules**: asset_path, cell, fs, handle, hotreload, loadable, map, meta, registry, service, services, stored, timeline.
- **Relations**: AssetCache loads Assets via AssetDesc/AsyncAssetKey; Resources are loaded via Loadable; Services provide loading backends; Assets are cached and shared.

#### ivy-derive
- **Purpose**: Procedural macros for deriving traits.
- **Key Macros**:
  - `#[derive(Resource)]`: Derives `Resource` trait, with `#[resource(load)]` for fields to load from assets.
  - `#[derive(Bundle)]`: Registers a `Resource` as a component Bundle, generates `BundleDesc` and inventory registration.
  - `#[derive(Editable)]`: For editor integration, with `#[editable]` attributes.
- **Modules**: bundle, editable, resource.
- **Relations**: Used to derive traits for structs in other crates, enabling asset loading and bundling.

#### ivy-editable
- **Purpose**: Components and traits for editor integration.
- **Key Components**:
  - `Editable` trait: For types that can be edited in the editor, creates UI widgets for editing.
  - `EditableWithOpts` trait: Editable with options like range.
  - `Projection` trait: For projecting state to sub-types, with `project`, `project_stream`, `write`.
  - `EditorOpts<T>`: Options for editors, e.g., range for numeric types.
  - Implementations for `Vec<T>`, `BTreeMap<K, V>`, and registered types like `String`, `f32`, etc.
- **Modules**: impls, registry.
- **Relations**: Used by editor to create UI for editing component values; integrates with Violet UI library.

### Rendering and Graphics

#### ivy-graphics
- **Purpose**: Graphics abstractions.
- **Key Components**:
  - Camera: `projection_matrix`, `environment_data`, `camera_settings` (with `CameraProjection`, `EnvironmentData`).
  - Mesh: `AttributeType` (U32, Vec3, etc.), `AttributeValues` for vertex data, `MeshData` for geometry.
  - Texture: Texture abstractions (likely in ivy-wgpu).
- **Modules**: camera, mesh, texture.
- **Relations**: Provides data structures for graphics; camera and mesh data used by rendering layers.

#### ivy-wgpu
- **Purpose**: WebGPU-based rendering backend.
- **Key Components**:
  - `Gpu`: GPU abstraction (from ivy-wgpu-types).
  - `RenderGraph`: Manages rendering nodes and resources; `Node` trait for render passes, `Dependency` for resource usage.
  - `RenderGraphResources`: Stores textures, buffers; `NodeExecutionContext`, `NodeUpdateContext` for node operations.
  - Renderers: `LightManager`, `ObjectManager`, `MeshRenderer`, `GizmosRenderer`, `Shadowmapping`.
  - Effects: `RenderEffect`, shader effects.
  - Materials, meshes, textures: GPU representations.
  - Shader library: `ShaderLibrary`, `RenderShader`.
- **Modules**: components, driver, effect, light, material, mesh, renderer (culling, gizmos_renderer, light_manager, mesh_renderer, object_manager, shadowmapping), rendergraph, shader, texture.
- **Relations**: Provides GPU rendering; integrates with ivy-graphics for data, ivy-core for ECS.

#### ivy-wgpu-types
- **Purpose**: Lower-level abstractions for Ivy-WGPU.
- **Key Components**:
  - `Gpu`: Main GPU interface, wraps wgpu device/queue.
  - `BufferAllocator`: Manages allocations in GPU buffers.
  - Shader types: `TargetDesc`, `RenderShader`, `TypedBuffer`.
  - Bind group builders: `BindGroupBuilder`, `BindGroupLayoutBuilder`.
- **Modules**: allocator, effect, shader, etc.
- **Relations**: Provides low-level GPU utilities for ivy-wgpu.

#### ivy-postprocessing
- **Purpose**: Post-processing effects.
- **Key Components**:
  - Preconfigured pipelines: `SurfacePbrRenderer`, `PbrRenderGraphConfig`, `PbrRenderGraphTextures`.
  - Effects: Bloom, tone mapping, etc.
- **Shaders**: Post-processing WGSL shaders (bloom_downsample, etc.).
- **Modules**: preconfigured (pbr), etc.
- **Relations**: Integrates with render graph for post-processing passes.

### Physics and Simulation

#### ivy-physics
- **Purpose**: Physics simulation using Rapier3D.
- **Key Components**:
  - `PhysicsState`: Wraps Rapier physics world.
  - Components: `rigidbody_builder`, `collider_builder`, `rb_handle`, `collider_handle`, `velocity`, `gravity`, `mass`, `effector`.
  - `Effector`: Trait for applying forces/torques.
  - `EntityCollisionEvent`: Collision events with entity info.
  - Bundles: Physics bundles for entities.
- **Modules**: bundles, components, effector, plugin, state, systems, shapes, util.
- **Relations**: Integrates with ECS; physics state updated in layers.

### Input and Interaction

#### ivy-input
- **Purpose**: Input handling system.
- **Key Components**:
  - `InputState`: Manages actions and handlers.
  - `Action<T>`: Binds inputs to stimuli, with `InputStimulus` trait (bool, f32, Vec2, etc.).
  - `InputEvent`: Keyboard, mouse, cursor events.
  - `Binding` trait for input mappings.
- **Modules**: bindings, components, error, layer, types.
- **Relations**: Processes winit events into actions that update ECS components or trigger callbacks.

### User Interface

#### ivy-ui
- **Purpose**: UI components built on Violet GUI library.
- **Key Components**:
  - `ui_instance`: Handle to Violet AppInstance.
  - `texture_dependency`: Texture for UI rendering.
  - `on_input_event`: Callback for input events.
- **Modules**: components, image, layer, node, screens, streamed, toast.
- **Relations**: Integrates Violet UI with Ivy's ECS and rendering.

### Asset Loading

#### ivy-gltf
- **Purpose**: GLTF asset loading and management.
- **Key Components**:
  - `Document`: Parsed GLTF with buffers, images, meshes, skins.
  - `DocumentData`: Internal data with assets.
  - `GltfNode`, `GltfMesh`, `GltfMaterial`, `GltfPrimitive`: Accessors for GLTF elements.
  - `GltfNodeDesc`: Descriptor for loading nodes.
  - Animation: Skin, animation data.
- **Modules**: animation (skin), components.
- **Relations**: Loads GLTF files into Ivy assets; provides access to meshes, materials, nodes for rendering/physics.

### Scene Management

#### ivy-scene
- **Purpose**: Cross-library scene management.
- **Key Components**:
  - `GltfNodeExt`: Mounts GLTF nodes to ECS entities with materials/effects.
  - `NodeMountOptions`: Options for mounting (material overrides, etc.).
  - Camera management, ray picking, colliders.
- **Modules**: camera, collider, drop, editor, ray_picker, scene.
- **Relations**: Bridges GLTF, physics, graphics, UI for scene setup.

### Game Logic

#### ivy-game
- **Purpose**: Game-specific logic and behaviors.
- **Key Components**:
  - Behavior trees, controllers, cameras (fly, orbit, viewport).
  - Lifetime management, navigation.
- **Modules**: behavior_tree, controllers, debug, fly_camera, lifetime, navigation, orbit_camera, standalone_camera, viewport_camera.
- **Relations**: Provides high-level game logic on top of core systems.

### Utilities

#### ivy-random
- **Purpose**: Random utility functions.
- **Features**: Random number generation using rand, for vectors, etc.

#### ivy-profiling
- **Purpose**: Profiling support (optional with Puffin).
- **Features**: `profile_function`, `profile_scope` macros.

#### ivy-reflect
- **Purpose**: Reflection (currently minimal).
- **Features**: Basic reflection traits (likely bevy_reflect integration).

### Editor

#### ivy-editor
- **Purpose**: Editor tools and integration.
- **Key Components**:
  - Tools: Gizmos, property editing.
  - Host, plugin, tools_controller, UI.
- **Modules**: host, plugin, tools, tools_controller, ui.
- **Relations**: Provides editor UI for modifying scenes/entities.

### External Dependencies

- **Violet**: Retained mode GUI library for UI.
- **Flax**: ECS library.
- **Rapier3D**: Physics engine.
- **WGPU**: WebGPU API.
- **Glam**: Math library (vectors, matrices).
- **Image**: Image loading/processing.

## Architecture

### App and Layers
The `App` struct manages:
- ECS `World` for entities and components.
- `AssetCache` for asset loading.
- `EventRegistry` for inter-layer communication.
- Layers: Modular logic units (e.g., rendering layer, physics layer).

Layers implement the `Layer` trait, handling initialization, updates, and events.

### ECS Components
Core components include transforms (position, rotation, scale), rendering (meshes, materials), physics (rigidbodies, colliders), and custom game components.

### Asset System
Assets are loaded asynchronously, cached, and shared. Supports hot reloading. Resources are for non-shared data.

### Rendering Pipeline
- Graphics layer handles rendering.
- WGPU backend provides GPU abstraction.
- Render graphs define rendering passes.
- Post-processing effects applied after main rendering.

### Physics Integration
Physics simulation runs in sync with the game loop. Entities can have physics components that interact with the Rapier world.

### Input System
Input is processed into actions that can update components or trigger callbacks. Supports various input types and bindings.

### UI System
Built on Violet, provides retained-mode GUI with components for layout, interaction, and rendering.

## Feature Availability

- **Rendering**: WebGPU-based, supports meshes, materials, lights, textures, shaders.
- **Physics**: Full 3D physics with Rapier3D, including rigidbodies, colliders, joints.
- **Audio**: Not included (external libraries recommended).
- **Networking**: Not included (layers can be added).
- **Scripting**: Not included (Rust-based).
- **Cross-platform**: Via WGPU and Winit.
- **Editor**: Basic editor with gizmos and property editing.
- **Asset Pipeline**: GLTF support, image loading, custom asset types.
- **Profiling**: Optional with Puffin integration.

## Functionality Highlights

- **Modular Design**: Crates can be used independently or together.
- **Async Asset Loading**: Non-blocking asset loading with futures.
- **Hot Reloading**: Assets reload automatically on file changes.
- **Gizmos**: Debug visualization for development.
- **Serialization**: Scene and component serialization.
- **Animation**: GLTF animation support.
- **Post-processing**: Effects like bloom, tone mapping.
- **Input Binding**: Flexible input mapping to game actions.
- **UI Integration**: Seamless UI overlay on 3D scenes.

This structure provides a comprehensive foundation for building complex 3D applications and games in Rust.</content>
</xai:function_call">The Structure.md file has been created with a comprehensive overview of the Ivy Engine's architecture, crates, and functionality. It outlines the main structure, key components, and available features based on the traversed codebase. The document covers core systems like ECS, rendering, physics, assets, and UI, providing a clear reference for understanding the engine's capabilities. 

If you need more details on specific crates, components, or examples, let me know! 

To complete the task, mark the final todo as done. 

<xai:function_call name="todowrite">
<parameter name="todos">[{"content":"Compile findings into a comprehensive Structure.md markdown file","status":"completed","priority":"high","id":"compile_structure_md"}]
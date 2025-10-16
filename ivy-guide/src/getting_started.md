# Getting Started
This section will guide you through installing Ivy Engine and creating your first application.

## Prerequisites
- Rust 1.70 or later
- A WebGPU-compatible GPU (most modern GPUs)

## Installation
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

## Quick Start
Here's a basic example that sets up a window with rendering, physics, and input:

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

This creates an application with:
- A window via WinitDriver
- Basic engine layer for core functionality
- PBR rendering with post-processing
- Input handling
- Physics simulation with gravity
- A fly camera for navigation
- Transform updates

## Running Examples
Ivy includes several examples to help you learn. Check out the `examples/` directory for:

- Basic rendering setup
- Physics simulations
- Input handling
- UI integration
- Full game examples

Run an example:

```bash
cargo run --example basic
```

## Next Steps
- Learn about [Layers](fundamentals/layers.md) for organizing your application logic
- Understand the [ECS](fundamentals/ecs.md) for game logic
- Explore [Rendering](fundamentals/rendering.md) for graphics
- Check out the [Crates](../crates/) section for detailed API documentation

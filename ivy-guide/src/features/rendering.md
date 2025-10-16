# Rendering & Graphics

Learn how to set up and use Ivy's rendering system for creating visually rich 3D applications.

## Setting Up a Basic Renderer
Start with a pre-configured PBR renderer for immediate results:

```rust
use ivy_postprocessing::preconfigured::{SurfacePbrPipelineDesc, SurfacePbrRenderer};

let renderer = GraphicsLayer::new(|world, assets, store, gpu, surface| {
    Ok(SurfacePbrRenderer::new(
        world, assets, store, gpu, surface,
        SurfacePbrPipelineDesc::default(),
    ))
});
```

## Creating Renderable Entities
Add meshes, materials, and transforms to make objects visible:

```rust
// Spawn an entity with rendering components
world.spawn()
    .set_bundle(TransformBundle {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    })
    .set(mesh(), mesh_handle)
    .set(material(), material_handle);
```

## Working with Materials
Load and customize PBR materials:

```rust
// Load a material from asset path
let material: Asset<Material> = assets.load("materials/my_material.mat").await?;

// Or create programmatically
let material = Material {
    albedo: Color::WHITE,
    metallic: 0.0,
    roughness: 0.5,
    ..Default::default()
};
```

## Lighting Your Scene
Add dynamic lights for realistic illumination:

```rust
use ivy_wgpu::components::*;

// Point light
world.spawn()
    .set_bundle(TransformBundle {
        position: Vec3::new(0.0, 5.0, 0.0),
        ..Default::default()
    })
    .set(light_params(), LightParams::new(Color::WHITE, 100.0))
    .set(light_kind(), LightKind::Point)
    .set(cast_shadow(), true);

// Directional light (sun)
world.spawn()
    .set_bundle(TransformBundle::default().with_rotation(Quat::from_rotation_ypr(0.5, -1.0, 0.3)))
    .set(light_params(), LightParams::new(Color::WHITE, 1.0))
    .set(light_kind(), LightKind::Directional)
    .set(cast_shadow(), true);
```
## Using Gizmos for Debugging

Visualize positions, vectors, and shapes during development:

```rust
// Draw debug shapes in your systems
fn debug_system(gizmos: Res<Gizmos>) {
    gizmos.sphere(Vec3::ZERO, 1.0, Color::RED);
    gizmos.line(Vec3::ZERO, Vec3::X, Color::GREEN);
    gizmos.cuboid(Vec3::ZERO, Vec3::ONE, Color::BLUE);
}
```
## Custom Render Graphs

For advanced rendering pipelines, build custom render graphs:

```rust
let render_graph = RenderGraph::new()
    .with_node(ShadowMapping::new())
    .with_node(MeshRenderer::new())
    .with_node(BloomEffect::new())
    .with_node(Tonemapping::new());
```
## Performance Tips

- Use instanced rendering for repeated objects
- Batch similar materials together
- Leverage LOD (Level of Detail) for distant objects
- Profile with render graph visualization tools
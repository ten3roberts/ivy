# Rendering and Passes

## Shaderpass

Compared to other game engines, Ivy uses a slightly more complicated, though more
flexible approach to rendering.

All rendereable entities, hereby referred to as *objects* have an associated
shaderpass. A shaderpass holds a shader and describes at which point it will be rendered
in the rendering pipelines.

Each node in the rendering shaderpass has an associated type of shaderpass which it will
render. For example, the `ImageRenderer` will usually be set up to render
objects which have a `Handle<ImagePass>`. The `ImagePass`, wrapped in an opaque
resource handle, will describe the vulkan pipeline and layout used.

**Note**: The renderer usually expect the different shaderpasses to conform to
a single pipeline layout due to descriptor binding.

Objects with meshes usually have a `GeometryPass` attached to them, with the
mesh and/or material describing the specific properties like texture and
roughness.

Different objects which belong to the same shaderpass can have different values, I.e;
different shaders, which for example can be used for wind affected foliage to be
rendered along other objects in the same shaderpass, but different shaders.


The system also allows for multiple shaderpasses to be attached to the same entity,
allowing the entity to use different shaders for different shaderpasses. This high
customizability allows the same entity to use a textured albedo shader for
`GeometryPass`, and a solid color for a hypothetical `MinimapPass`. This can be
very useful in games where the same object may be required to be rendered
multiple times from different viewpoints.

A `ShaderPass` is a type which wraps a `Pipeline` and a `PipelineLayout`, though
they can contain other info. The Rust type system is used for differentiating
between different kinds of passes.

For reducing boilerplate a convenience macro `new_shaderpass` is provided for
easily creating one or more stronly typed shaderpass types.

Example:
```rust
use ivy::new_shaderpass;


new_shaderpass! {
  pub struct MinimapPass;
  pub struct SolidPass;
}
```

In many cases though, the usage of the included `GeometryPass`, `ImagePass`,
`TextPass`, and different post processing passes are enough.

## Rendergraph

The rendering graph describes an acyclic graph of rendering nodes, which
describe how the scene will be rendered. Each node describes its inputs and
outputs, and dependencies will automatically be generated to ensure proper
ordering and syncronization with paralellization using Vulkan.

`ivy-presets` contain common rendergraph setups, such as for PBR rendering. It
is also possible to create your own rendergraph to tailor the rendering for your
game or application.

The following example shows the raw, unaided setup of a rendergraph rendering a
simple unlit model to the screen.

```rust
{{ #include ../../../examples/rendergraph.rs }}
```

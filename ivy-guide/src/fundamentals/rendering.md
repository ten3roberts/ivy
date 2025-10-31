# Rendering and Passes
Ivy uses a modern, flexible rendering system based on WebGPU and render graphs.

## Render Graph System
The rendering graph describes an acyclic graph of rendering nodes that define how the scene is rendered. Each node describes its inputs and outputs, with dependencies automatically generated for proper ordering and synchronization.

Render graphs enable:
- Multi-pass rendering pipelines
- Parallel execution where possible
- Flexible rendering architectures
- Post-processing integration

## Shader Passes
Renderable entities have associated shader passes that determine when and how they render. A shader pass wraps a GPU pipeline and describes rendering parameters.

Common shader passes include:
- **GeometryPass**: Standard 3D geometry with materials
- **ImagePass**: 2D image rendering
- **TextPass**: Text rendering
- **ShadowPass**: Shadow map generation

## Materials and Shaders
Materials define surface properties (albedo, roughness, metallic, etc.) and reference shaders. Ivy supports:

- Physically Based Rendering (PBR) materials
- Custom WGSL shaders
- Material instancing for performance
- Texture mapping and sampling

## Lighting
The lighting system supports:
- Point lights, directional lights, spot lights
- Shadow mapping
- Light culling and optimization
- Dynamic light properties

## Post-Processing
Post-processing effects are applied after main rendering:
- Bloom and glow effects
- HDR tonemapping
- Color grading
- Custom effects via render graph nodes

## Example: Basic Render Graph Setup
```rust
use ivy_wgpu::{rendergraph::RenderGraph, renderer::MeshRenderer};

// Create a render graph with PBR rendering
let render_graph = RenderGraph::new()
    .with_node(MeshRenderer::new(...))
    .with_node(ShadowRenderer::new(...))
    .with_node(PostProcessingNode::new(...));
```

## Performance Considerations
- Use instancing for repeated objects
- Batch draw calls where possible
- Leverage render graph parallelism
- Profile with GPU debugging tools

The render graph system provides the flexibility to create custom rendering pipelines while maintaining high performance.

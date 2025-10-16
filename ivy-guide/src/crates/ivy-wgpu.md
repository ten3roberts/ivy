# ivy-wgpu

WebGPU-based rendering backend.
## Key Components

### RenderGraph
Manages rendering nodes and resources; `Node` trait for render passes, `Dependency` for resource usage.

### Renderers
- `LightManager`: Lighting management
- `ObjectManager`: Object rendering
- `MeshRenderer`: Mesh rendering
- `GizmosRenderer`: Debug gizmos
- `Shadowmapping`: Shadow rendering
## Modules

- `components`: Rendering components
- `driver`: WGPU driver setup
- `effect`: Rendering effects
- `light`: Lighting systems
- `material`: Material management
- `mesh`: Mesh handling
- `renderer`: Various renderers (culling, gizmos_renderer, light_manager, mesh_renderer, object_manager, shadowmapping)
- `rendergraph`: Render graph system
- `shader`: Shader management
- `texture`: Texture handling
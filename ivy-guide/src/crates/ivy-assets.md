# ivy-assets
Sync and async asset system with caching.

## Key Components
### AssetCache
Central store for assets, manages loading, caching, and services.

### Asset<T>
Shared handle to loaded assets, wraps `Arc<T>` with `AssetId`.

### AssetLoadFuture<T>
Future for async asset loading.

### Resource Trait
For types that can be loaded from descriptions.

### Loadable Trait
Generic loading mechanism.

### Services
- `FileSystemMapService`: File access
- Hot reloading via `FileReloadService`

### AssetPath<T>
Path wrapper for typed asset loading.

## Modules
- `asset_path`: Asset path utilities
- `cell`: Asset storage cells
- `fs`: File system services
- `handle`: Asset handles
- `hotreload`: Hot reloading
- `loadable`: Loading traits
- `map`: Asset mapping
- `meta`: Asset metadata
- `registry`: Asset registry
- `service`: Loading services
- `services`: Service implementations
- `stored`: Stored assets
- `timeline`: Asset timelines

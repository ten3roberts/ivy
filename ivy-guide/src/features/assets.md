# Asset Pipeline
Master Ivy's asset management system for efficient loading, caching, and hot reloading of game resources.

## Loading Assets
Use the asset cache to load resources asynchronously:

```rust
async fn load_assets(assets: Res<AssetCache>) -> anyhow::Result<()> {
    // Load a texture
    let texture: Asset<Texture> = assets.load("textures/player.png").await?;

    // Load a 3D model
    let model: Asset<GltfDocument> = assets.load("models/character.gltf").await?;

    // Load a custom asset type
    let config: Asset<GameConfig> = assets.load("config/game.toml").await?;

    Ok(())
}

## Using Assets in Entities
Reference loaded assets in your entity spawning:

```rust
fn spawn_player(
    mut commands: Commands,
    assets: Res<AssetCache>,
) {
    let texture = assets.load_sync::<Texture>("textures/player.png").unwrap();

    commands.spawn()
        .insert_bundle(SpriteBundle {
            texture,
            transform: Transform::from_xyz(0.0, 0.0, 0.0),
            ..Default::default()
        });
}
```

## Hot Reloading
Assets automatically reload when files change during development:

```rust
// No special code needed - just load assets normally
let material = assets.load::<Material>("materials/wall.mat").await?;

// When you modify wall.mat on disk, the asset automatically updates
// All references to the material will use the new version
```

## Custom Asset Types
Define your own asset types with custom loading logic:

```rust

#[derive(Resource)]
struct GameConfig {
    player_speed: f32,
    enemy_count: u32,
}

impl Loadable for GameConfig {
    type Desc = AssetPath<GameConfig>;

    async fn load(desc: Self::Desc, ctx: &mut LoadContext) -> anyhow::Result<Self> {
        // Load from TOML file
        let content = ctx.read_to_string(desc.path()).await?;
        let config: GameConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
```

## Asset Dependencies
Handle assets that depend on other assets:

```rust

#[derive(Resource)]
struct CharacterModel {
    mesh: Asset<Mesh>,
    material: Asset<Material>,
    animations: Vec<Asset<Animation>>,
}

impl Loadable for CharacterModel {
    async fn load(desc: AssetPath<Self>, ctx: &mut LoadContext) -> anyhow::Result<Self> {
        // Load GLTF and extract components
        let gltf = ctx.load::<GltfDocument>("character.gltf").await?;

        Ok(CharacterModel {
            mesh: gltf.meshes[0].clone(),
            material: gltf.materials[0].clone(),
            animations: gltf.animations.clone(),
        })
    }
}
```

## Asset Caching Strategies
Control how assets are cached and shared:

```rust
// Shared assets (default) - same asset reused across entities
let shared_texture = assets.load::<Texture>("shared.png").await?;

// Unique assets - each load creates a new instance
let unique_config = assets.load_unique::<GameConfig>("config.toml").await?;
```

## Asset Preloading
Load assets upfront to avoid loading stalls:

```rust
async fn preload_assets(assets: Res<AssetCache>) {
    // Start loading critical assets
    let _ = assets.load::<Texture>("ui/loading.png");
    let _ = assets.load::<Audio>("audio/bgm.ogg");

    // Wait for completion
    assets.wait_for_pending().await;
}
```

## Asset Organization
Structure your asset directories for maintainability:

```
assets/
├── textures/
│   ├── characters/
│   ├── environments/
│   └── ui/
├── models/
│   ├── props/
│   └── characters/
├── audio/
│   ├── sfx/
│   └── music/
├── materials/
└── config/
```

## Performance Tips
- Preload critical assets at startup
- Use asset references instead of storing asset data directly
- Monitor asset loading times with profiling tools
- Use compression for large assets
- Implement LOD for textures and models

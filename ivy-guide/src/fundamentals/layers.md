# Layers

Layers are the fundamental building blocks of Ivy applications, providing modular units of logic that can be stacked and configured.

## What are Layers?

Layers implement the `Layer` trait, providing methods for initialization (`register`), updates, and event handling. Each layer defines its own state and governs its own execution.

Layers are *mostly* self-contained units of logic that get exclusive access to the ECS world, resources, and events. They can execute logic in their update methods while maintaining isolation from other layers.

## Benefits of Layered Architecture

- **Modularity**: Logic can be added or removed without affecting other systems
- **Separation of Concerns**: Different aspects (rendering, physics, input) stay isolated
- **Composability**: Layers can be mixed and matched for different application types
- **Client/Server Sharing**: Same game logic can be used for both client and server with different layers

## Common Layer Types
- **Rendering Layer**: Handles GPU operations and graphics
- **Physics Layer**: Manages physics simulation
- **Input Layer**: Processes user interactions
- **UI Layer**: Manages user interface
- **Game Logic Layer**: Implements specific game rules
- **Network Layer**: Handles multiplayer communication
## Layer Lifecycle
Layers go through several phases:

1. **Registration**: Set up systems, events, and initial state
2. **Update**: Execute main logic each frame
3. **Event Handling**: Respond to system events
4. **Cleanup**: Release resources when the app shuts down

## Example: Custom Game Layer
```rust
use ivy_core::{layer::Layer, app::App, events::EventRegistry};
use flax::World;

struct GameLayer {
    score: i32,
}

impl Layer for GameLayer {
    fn register(
        &mut self,
        world: &mut World,
        events: &mut EventRegistry,
    ) -> anyhow::Result<()> {
        // Set up game systems and initial state
        Ok(())
    }

    fn update(
        &mut self,
        world: &World,
        events: &EventRegistry,
    ) -> anyhow::Result<()> {
        // Update game logic
        self.score += 1;
        Ok(())
    }
}
```

## Using Layers in App Builder
```rust
use ivy_core::App;

let app = App::builder()
    .with_layer(GameLayer { score: 0 })
    .with_layer(PhysicsLayer::new())
    .with_layer(RenderLayer::new())
    .run()?;
```

Layers provide the foundation for building complex, maintainable applications with clear separation of concerns.

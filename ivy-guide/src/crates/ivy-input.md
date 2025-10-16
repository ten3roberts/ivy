# ivy-input
Input handling system.

## Key Components
### InputState
Manages actions and handlers.

### Action<T>
Binds inputs to stimuli, with `InputStimulus` trait (bool, f32, Vec2, etc.).

### InputEvent
Keyboard, mouse, cursor events.

### Binding Trait
For input mappings.

## Modules
- `bindings`: Input binding system
- `components`: Input components
- `error`: Error handling
- `layer`: Input layer
- `types`: Input types

# Input & Interaction
Master Ivy's flexible input system for responsive, customizable controls in your applications.

## Setting Up Input Handling
Add the input layer to your app:

```rust
use ivy_input::layer::InputLayer;

app.with_layer(InputLayer::new());

## Defining Actions
Create actions that map inputs to game logic:

```rust
use ivy_input::{Action, InputStimulus};

// Movement action (WASD or left stick)
let move_action = Action::<Vec2>::new("move")
    .with_binding(KeyCode::KeyW, Vec2::Y)
    .with_binding(KeyCode::KeyS, -Vec2::Y)
    .with_binding(KeyCode::KeyA, -Vec2::X)
    .with_binding(KeyCode::KeyD, Vec2::X)
    .with_binding(GamepadAxis::LeftStickX, |value| Vec2::new(value, 0.0))
    .with_binding(GamepadAxis::LeftStickY, |value| Vec2::new(0.0, value));

// Jump action (space or A button)
let jump_action = Action::<bool>::new("jump")
    .with_binding(KeyCode::Space, true)
    .with_binding(GamepadButton::South, true);
```

## Using Actions in Systems
Read action values in your game logic:

```rust

#[system]
fn player_movement(
    mut transforms: Query<&mut Transform>,
    move_input: Res<Action<Vec2>>,
    jump_input: Res<Action<bool>>,
    time: Res<Time>,
) {
    let move_vector = move_input.value();
    let should_jump = jump_input.value();

    for mut transform in &mut transforms {
        // Apply movement
        let movement = Vec3::new(move_vector.x, 0.0, move_vector.y) * time.delta_seconds();
        transform.translation += movement;

        // Handle jumping
        if should_jump {
            // Apply jump force
        }
    }
}
```

## Custom Input Processing
Create custom input handlers for specific needs:

```rust
struct CustomInputHandler {
    sensitivity: f32,
}

impl InputHandler for CustomInputHandler {
    fn process_event(&mut self, event: &InputEvent, actions: &mut ActionState) {
        match event {
            InputEvent::MouseMotion { delta } => {
                // Apply mouse look with custom sensitivity
                let look_delta = *delta * self.sensitivity;
                actions.set_action("look", Vec2::new(look_delta.x, look_delta.y));
            }
            _ => {}
        }
    }
}
```

## Gamepad Vibration
Provide haptic feedback:

```rust
fn gamepad_feedback(
    gamepads: Res<GamepadState>,
    mut events: EventWriter<GamepadRumbleEvent>,
) {
    for (id, gamepad) in gamepads.iter() {
        // Rumble on collision
        events.send(GamepadRumbleEvent {
            gamepad: *id,
            left_motor: 0.5,
            right_motor: 0.5,
            duration: Duration::from_millis(200),
        });
    }
}
```

## Input Configuration
Allow players to customize controls:

```rust
// Load/save input bindings
let config = InputConfig::load_from_file("controls.json")?;
app.with_input_config(config);

// Or modify at runtime
app.modify_input_bindings(|bindings| {
    bindings.add_binding("move", KeyCode::ArrowUp, Vec2::Y);
});
```

## Best Practices
- Use action names consistently across your codebase
- Provide sensible defaults for all actions
- Support both keyboard/mouse and gamepad input
- Allow rebinding for accessibility
- Test with multiple input devices

# User Interface
Build interactive user interfaces that integrate seamlessly with your 3D Ivy applications using the Violet GUI library.

## Setting Up UI
Ivy uses the Violet UI library for user interfaces. Add the UI layer to your application:

```rust
use ivy_ui::layer::UiLayer;

app.with_layer(UiLayer::new());
```

## Creating UI Elements
Violet provides a widget-based UI system. For detailed examples, see the violet examples in the violet/ directory.

Basic ECS-based UI components from violet-core can be used:

```rust
use violet_core::components::*;

// Create a basic UI element
world.spawn()
    .set(rect, violet_core::Rect::new(100.0, 100.0, 200.0, 50.0))
    .set(color, palette::Srgba::new(0.0, 0.5, 1.0, 1.0));

// Create text
world.spawn()
    .set(rect, violet_core::Rect::new(100.0, 200.0, 200.0, 30.0))
    .set(text, vec![violet_core::text::TextSegment::new("Hello, World!")])
    .set(font_size, 24.0);
```

## Handling UI Events
Violet handles input and events through its widget system. For event handling, see violet examples and documentation.

## Layout and Positioning
Violet provides layout systems for positioning UI elements. Use components like `anchor`, `margin`, `padding` from violet-core for layout.

## Styling and Themes
Violet uses style components for theming. Use `color`, `font_size`, etc., components for styling.

## Integrating with 3D Scenes
UI renders as an overlay on your 3D content:

```rust
// UI automatically appears on top of 3D rendering
// No special setup required - just add UiLayer after GraphicsLayer
app
    .with_layer(GraphicsLayer::new(renderer))
    .with_layer(UiLayer::new()); // UI renders on top
```

## Custom Widgets
Violet allows creating custom widgets. For custom components, define your own components and systems to update UI state.

## Performance Tips
- Use UI layers sparingly for complex interfaces
- Batch similar widgets together
- Avoid updating UI transforms every frame
- Use object pooling for dynamic UI elements

use glfw::{Action, Key, Modifiers};
use hecs::{Bundle, DynamicBundleClone, DynamicClone, Entity};
use hecs_schedule::{Read, SubWorld, Write};

use ivy_base::Events;

use crate::{
    events::WidgetEvent, Interactive, InteractiveState, Result, Sticky, Text, WidgetEventKind,
};

/// A bundle for spawning an input field.
///
/// It is recommended to also include a TextBundle and a ImageBundle, as well as
/// appropriate passes.
#[derive(Default, Bundle, Clone, DynamicBundleClone)]

pub struct InputFieldBundle {
    pub interactive: Interactive,
    pub sticky: Sticky,
    pub field: InputField,
}

impl Default for InputField {
    fn default() -> Self {
        Self {
            on_submit: |_, _, _| (),
        }
    }
}

#[derive(Clone, Copy)]
pub struct InputField {
    pub on_submit: fn(Entity, &mut Events, value: &str),
}

impl InputField {
    pub fn new(on_submit: fn(Entity, &mut Events, value: &str)) -> Self {
        Self { on_submit }
    }
}

pub fn input_field_system(
    world: SubWorld<(&mut InputField, &mut Text)>,
    state: Read<InteractiveState>,
    reader: impl Iterator<Item = WidgetEvent>,
    mut events: Write<Events>,
) -> Result<()> {
    let focused = match state.focused() {
        Some(val) => val,
        None => return Ok(()),
    };

    let mut query = world.query_one::<(&mut InputField, &mut Text)>(focused)?;
    let (field, text) = match query.get().ok() {
        Some(val) => val,
        None => return Ok(()),
    };

    reader.for_each(|event| match event.kind {
        WidgetEventKind::Focus(false)
        | WidgetEventKind::Key {
            key: Key::Enter,
            action: Action::Press,
            ..
        } => {
            (field.on_submit)(focused, &mut events, text.val());
        }
        WidgetEventKind::CharTyped(c) => {
            text.append(c);
        }
        WidgetEventKind::Key {
            key: Key::Backspace,
            scancode: _,
            action: Action::Repeat | Action::Press,
            mods: Modifiers::Control,
        } => {
            text.remove_back_word();
        }
        WidgetEventKind::Key {
            key: Key::Backspace,
            scancode: _,
            action: Action::Repeat | Action::Press,
            mods: _,
        } => {
            text.remove_back();
        }
        _ => {}
    });

    Ok(())
}

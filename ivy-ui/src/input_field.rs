use flax::{Entity, EntityBuilder, Mutable, Query, QueryBorrow, System};
use glfw::{Action, Key, Modifiers};

use ivy_base::Events;

use crate::{
    events::WidgetEvent, input_field, interactive, sticky, text, InteractiveState, Result, Text,
    WidgetEventKind,
};

/// A bundle for spawning an input field.
///
/// It is recommended to also include a TextBundle and a ImageBundle, as well as
/// appropriate passes.
#[derive(Default, Clone)]
pub struct InputFieldBundle {
    // pub interactive: Interactive,
    // pub sticky: Sticky,
    pub field: InputField,
}

impl InputFieldBundle {
    pub fn new(field: InputField) -> Self {
        Self { field }
    }

    pub fn mount(&self, entity: &mut EntityBuilder) {
        entity
            .set_default(interactive())
            .set_default(sticky())
            .set(input_field(), self.field)
    }
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
    // world: SubWorld<(&mut InputField, &mut Text)>,
    // state: Read<InteractiveState>,
    reader: flume::Receiver<WidgetEvent>,
    // mut events: Write<Events>,
) -> Result<()> {
    System::builder()
        .with_query(Query::new((input_field().as_mut(), text().as_mut())))
        .with_input::<InteractiveState>()
        .with_input_mut::<Events>()
        .build(
            |mut query: QueryBorrow<(Mutable<InputField>, Mutable<Text>)>,
             state: &mut InteractiveState,
             events: &mut Events| {
                let focused = match state.focused() {
                    Some(val) => val,
                    None => return Ok(()),
                };

                let Ok((field, text)) = query.get(focused) else {
                    return Ok(());
                };
                // let (field, text) = match query.get().ok() {
                //     Some(val) => val,
                //     None => return Ok(()),
                // };

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
            },
        )
        .boxed()
}

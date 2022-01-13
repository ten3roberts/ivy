use glfw::{Action, Key, Modifiers};
use hecs::{Component, DynamicBundleClone, Entity, EntityBuilderClone, World};
use hecs_hierarchy::*;
use hecs_schedule::{Read, SubWorld};

use ivy_resources::Handle;

use crate::{
    constraints::RelativeSize, events::WidgetEvent, ImageBundle, Interactive, InteractiveState,
    Result, Sticky, Text, TextBundle, Widget, WidgetBundle, WidgetEventKind,
};

/// A bundle for easily creating input fields with a reactive component
pub struct InputFieldInfo<T: Component, U: Component, W> {
    pub text: TextBundle,
    pub text_pass: Handle<T>,
    pub background_pass: Handle<U>,
    pub widget: W,
    pub background: ImageBundle,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct InputField;

impl InputField {
    /// Creates a new input field.
    pub fn spawn<T: Component, U: Component, W: DynamicBundleClone>(
        world: &mut World,
        info: InputFieldInfo<T, U, W>,
    ) -> Entity {
        Self::build_tree(info).spawn(world)
    }

    pub fn build_tree<T: Component, U: Component, W: DynamicBundleClone>(
        info: InputFieldInfo<T, U, W>,
    ) -> DeferredTreeBuilder<Widget> {
        let mut builder = DeferredTreeBuilder::<Widget>::new();

        builder
            .add_bundle(info.widget)
            .add_bundle(info.text)
            .add(info.text_pass)
            .add_bundle((Interactive, Sticky, InputField));

        builder.attach_new({
            let mut builder = EntityBuilderClone::new();
            builder
                .add_bundle(info.background)
                .add_bundle(WidgetBundle {
                    rel_size: RelativeSize::new(1.0, 1.0),
                    ..Default::default()
                })
                .add(info.background_pass);
            builder
        });

        builder
    }
}

pub fn input_field_system(
    world: SubWorld<(&mut InputField, &mut Text)>,
    state: Read<InteractiveState>,
    events: impl Iterator<Item = WidgetEvent>,
) -> Result<()> {
    let focused = match state.focused() {
        Some(val) => val,
        None => return Ok(()),
    };

    let mut query = world.query_one::<(&mut InputField, &mut Text)>(focused)?;
    let (_, text) = match query.get().ok() {
        Some(val) => val,
        None => return Ok(()),
    };

    events.for_each(|event| match event.kind {
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

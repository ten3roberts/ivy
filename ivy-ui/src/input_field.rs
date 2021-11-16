use fontdue::layout::{HorizontalAlign, VerticalAlign};
use glfw::{Action, Key, Modifiers};
use hecs::{Component, Entity, World};
use hecs_hierarchy::Hierarchy;
use ivy_input::InputEvent;
use ivy_resources::Handle;
use ultraviolet::Vec2;

use crate::{
    constraints::{AbsoluteOffset, AbsoluteSize, Origin2D, RelativeOffset, RelativeSize},
    events::WidgetEvent,
    Font, Image, Interactive, Reactive, Result, Sticky, Text, TextAlignment, Widget, WrapStyle,
};

pub struct InputField {
    text: Entity,
}

/// A bundle for easily creating input fields with a reactive component
#[derive(Debug)]
pub struct InputFieldInfo<T, U, G> {
    pub text_pass: Handle<U>,
    pub image_pass: Handle<G>,
    pub font: Handle<Font>,
    pub reactive: Reactive<T>,
    pub background: Handle<Image>,
    pub rel_size: RelativeSize,
    pub rel_offset: RelativeOffset,
    pub abs_size: AbsoluteSize,
    pub abs_offset: AbsoluteOffset,
    pub origin: Origin2D,
    pub text_padding: Vec2,
}

impl<T: Default, U, G> Default for InputFieldInfo<T, U, G> {
    fn default() -> Self {
        Self {
            text_pass: Default::default(),
            image_pass: Default::default(),
            font: Default::default(),
            reactive: Default::default(),
            background: Default::default(),
            rel_size: Default::default(),
            rel_offset: Default::default(),
            abs_size: Default::default(),
            abs_offset: Default::default(),
            origin: Default::default(),
            text_padding: Default::default(),
        }
    }
}

impl InputField {
    /// Creates a new input field
    pub fn spawn<T: Component, U: Component, G: Component>(
        world: &mut World,
        root: Entity,
        info: InputFieldInfo<T, U, G>,
    ) -> Result<Entity> {
        let text = world.spawn((
            Widget,
            RelativeOffset::new(0.0, 0.0),
            AbsoluteSize(-info.text_padding),
            RelativeSize::new(1.0, 1.0),
            info.text_pass,
            Text::default(),
            info.font,
            WrapStyle::Overflow,
            TextAlignment::new(HorizontalAlign::Right, VerticalAlign::Middle),
        ));

        let field = world.attach_new::<Widget, _>(
            root,
            (
                Widget,
                Sticky,
                InputField { text },
                Interactive,
                info.origin,
                info.image_pass,
                info.background,
                info.abs_size,
                info.abs_offset,
                info.rel_size,
                info.rel_offset,
                info.reactive,
            ),
        )?;

        world.attach::<Widget>(text, field)?;

        Ok(field)
    }
}

pub fn input_field_system<I: Iterator<Item = WidgetEvent>>(
    world: &World,
    events: I,
    active: Entity,
) -> Result<()> {
    if let Some(field) = world.get::<InputField>(active).ok() {
        let mut text = world.get_mut::<Text>(field.text)?;

        events.for_each(|event| match event.kind {
            InputEvent::CharTyped(c) => {
                text.append(c);
            }
            InputEvent::Key {
                key: Key::Backspace,
                scancode: _,
                action: Action::Repeat | Action::Press,
                mods: Modifiers::Control,
            } => text.remove_back_word(),
            InputEvent::Key {
                key: Key::Backspace,
                scancode: _,
                action: Action::Repeat | Action::Press,
                mods: _,
            } => text.remove_back(),
            _ => {}
        })
    }

    Ok(())
}

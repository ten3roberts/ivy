use std::borrow::Cow;

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

/// A bundle for easily creating input fields with a reactive component
#[derive(Debug)]
pub struct InputFieldInfo<T, U, G> {
    pub placeholder: Cow<'static, str>,
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
            placeholder: Default::default(),
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

pub struct InputField {
    text: Entity,
    val: Cow<'static, str>,
    placeholder: Cow<'static, str>,
}

impl InputField {
    pub fn new(text: Entity, val: Cow<'static, str>, placeholder: Cow<'static, str>) -> Self {
        Self {
            text,
            val,
            placeholder,
        }
    }

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
            Text::new(info.placeholder.to_owned()),
            info.font,
            WrapStyle::Overflow,
            TextAlignment::new(HorizontalAlign::Right, VerticalAlign::Middle),
        ));

        let field = world.attach_new::<Widget, _>(
            root,
            (
                Widget,
                Sticky,
                InputField::new(text, "".into(), info.placeholder),
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

    /// Returns the entered value of placeholder
    pub fn val(&self) -> &Cow<'static, str> {
        if self.val.is_empty() {
            &self.placeholder
        } else {
            &self.val
        }
    }

    fn sync(&self, text: &mut Text) {
        let src = self.val();
        text.val_mut().clone_from(src);
    }

    fn append(&mut self, ch: char) {
        dbg!(&self.val);
        let s = self.val.to_mut();
        s.push(ch);
    }
    /// Removes the last word
    fn remove_back_word(&mut self) {
        let s = self.val.to_mut();

        if s.len() == 0 {
            return;
        }

        let mut first = true;

        while let Some(c) = s.pop() {
            if !first && !c.is_alphanumeric() {
                s.push(c);
                break;
            }
            first = false;
        }
    }

    /// Removes the last char
    fn remove_back(&mut self) {
        let s = self.val.to_mut();
        s.pop();
    }
}

pub fn input_field_system<I: Iterator<Item = WidgetEvent>>(
    world: &World,
    events: I,
    active: Entity,
) -> Result<()> {
    if let Some(mut field) = world.get_mut::<InputField>(active).ok() {
        let mut text = world.get_mut::<Text>(field.text)?;
        events.for_each(|event| match event.kind {
            InputEvent::CharTyped(c) => {
                field.append(c);
                field.sync(&mut text);
            }
            InputEvent::Key {
                key: Key::Backspace,
                scancode: _,
                action: Action::Repeat | Action::Press,
                mods: Modifiers::Control,
            } => {
                field.remove_back_word();
                field.sync(&mut text)
            }
            InputEvent::Key {
                key: Key::Backspace,
                scancode: _,
                action: Action::Repeat | Action::Press,
                mods: _,
            } => {
                field.remove_back();
                field.sync(&mut text)
            }
            _ => {}
        })
    }

    Ok(())
}

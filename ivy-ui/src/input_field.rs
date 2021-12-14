use std::borrow::Cow;

use glfw::{Action, Key, Modifiers};
use hecs::{Component, Entity, EntityBuilder, World};
use hecs_hierarchy::*;
use hecs_schedule::{GenericWorld, Read, SubWorld};

use ultraviolet::Vec2;

use crate::{
    constraints::{AbsoluteSize, RelativeSize},
    events::WidgetEvent,
    Interactive, InteractiveState, Result, Sticky, Text, TextBundle, Widget, WidgetBundle,
    WidgetEventKind,
};

/// A bundle for easily creating input fields with a reactive component
pub struct InputFieldInfo<T> {
    pub text: TextBundle<T>,
    /// Specifies a builder for the field widget. In order to be rendered, the
    /// builder *should* contain atleast a [ `crate::WidgetBundle` ] and a renderable bundle
    /// such as [ `crate::ImageBundle` ].
    pub field: EntityBuilder,
    /// Placeholder text
    pub placeholder: Cow<'static, str>,
    pub text_padding: Vec2,
}

pub struct InputField {
    text: Entity,
    val: Cow<'static, str>,
    placeholder: Cow<'static, str>,
}

impl InputField {
    pub fn new(text: Entity, placeholder: Cow<'static, str>) -> Self {
        Self {
            text,
            val: Cow::Borrowed(""),
            placeholder,
        }
    }

    /// Creates a new input field
    pub fn spawn<T: Component>(
        world: &mut World,
        root: Entity,
        mut info: InputFieldInfo<T>,
    ) -> Result<Entity> {
        let mut builder = EntityBuilder::new();
        info.text.text.set(info.placeholder.clone());

        builder
            .add_bundle(WidgetBundle {
                abs_size: AbsoluteSize(-info.text_padding),
                rel_size: RelativeSize::new(1.0, 1.0),
                ..Default::default()
            })
            .add_bundle(info.text);

        let text = world.spawn(builder.build());

        let mut builder = info.field;
        builder.add_bundle((Interactive, Sticky, InputField::new(text, info.placeholder)));

        let field = world.attach_new::<Widget, _>(root, builder.build())?;

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

pub fn input_field_system(
    world: SubWorld<(&mut InputField, &mut Text)>,
    state: Read<InteractiveState>,
    events: impl Iterator<Item = WidgetEvent>,
) -> Result<()> {
    let focused = match state.focused() {
        Some(val) => val,
        None => return Ok(()),
    };

    let mut field = match world.try_get_mut::<InputField>(focused).ok() {
        Some(field) => field,
        None => return Ok(()),
    };

    let mut text = world.try_get_mut::<Text>(field.text)?;
    events.for_each(|event| match event.kind {
        WidgetEventKind::CharTyped(c) => {
            field.append(c);
            field.sync(&mut text);
        }
        WidgetEventKind::Key {
            key: Key::Backspace,
            scancode: _,
            action: Action::Repeat | Action::Press,
            mods: Modifiers::Control,
        } => {
            field.remove_back_word();
            field.sync(&mut text)
        }
        WidgetEventKind::Key {
            key: Key::Backspace,
            scancode: _,
            action: Action::Repeat | Action::Press,
            mods: _,
        } => {
            field.remove_back();
            field.sync(&mut text)
        }
        _ => {}
    });

    Ok(())
}

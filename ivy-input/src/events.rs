use std::path::PathBuf;

use flax::{component, EntityRef};
use glam::{vec2, IVec2, Vec2};
use glfw::{Action, Key, Modifiers, MouseButton, Scancode, WindowEvent};
use ivy_base::Extent;

/// Window input events
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// Window moved event
    Pos(IVec2),
    /// Window resize event
    Size(Extent),
    /// Key input event
    Key {
        key: Key,
        scancode: Scancode,
        action: Action,
        mods: Modifiers,
    },
    /// Scroll wheel event in horizontal and vertical
    Scroll(Vec2),
    /// A typed char with applied modifiers
    CharTyped(char),
    CharModifiers {
        c: char,
        mods: Modifiers,
    },
    /// Cursor moved
    CursorPos(Vec2),
    /// Cursor entered window
    CursorEnter(bool),
    /// Window recieved or lost focus
    Focus(bool),
    /// Window close event
    Close,
    MouseButton {
        button: MouseButton,
        action: Action,
        mods: Modifiers,
    },
    Iconify(bool),
    FileDrop(Vec<PathBuf>),
    FramebufferSize(Extent),
    Refresh,
    Maximize(bool),
    ContentScale(Vec2),
}

impl From<WindowEvent> for InputEvent {
    fn from(val: WindowEvent) -> Self {
        match val {
            WindowEvent::Pos(x, y) => Self::Pos(IVec2::new(x, y)),
            WindowEvent::Size(x, y) => Self::Size(Extent::new(x as _, y as _)),
            WindowEvent::Close => Self::Close,
            WindowEvent::Refresh => Self::Refresh,
            WindowEvent::Focus(val) => Self::Focus(val),
            WindowEvent::Iconify(val) => Self::Iconify(val),
            WindowEvent::FramebufferSize(x, y) => {
                Self::FramebufferSize(Extent::new(x as _, y as _))
            }
            WindowEvent::MouseButton(a, b, c) => Self::MouseButton {
                button: a,
                action: b,
                mods: c,
            },
            WindowEvent::CursorPos(x, y) => Self::CursorPos(vec2(x as _, y as _)),
            WindowEvent::CursorEnter(val) => Self::CursorEnter(val),
            WindowEvent::Scroll(x, y) => Self::Scroll(Vec2::new(x as _, y as _)),
            WindowEvent::Key(key, scancode, action, mods) => Self::Key {
                key,
                scancode,
                action,
                mods,
            },
            WindowEvent::Char(c) => Self::CharTyped(c),
            WindowEvent::CharModifiers(c, mods) => Self::CharModifiers { c, mods },
            WindowEvent::FileDrop(path) => Self::FileDrop(path),
            WindowEvent::Maximize(val) => Self::Maximize(val),
            WindowEvent::ContentScale(x, y) => Self::ContentScale(Vec2::new(x, y)),
        }
    }
}

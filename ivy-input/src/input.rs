use std::ops::Deref;

use flume::Receiver;
use glfw::{Action, Key, MouseButton};
use ivy_base::{Events, Extent, Position2D};
use ivy_graphics::Window;
use ultraviolet::Vec2;

use crate::events::InputEvent;

pub const MAX_KEYS: usize = glfw::Key::Menu as usize;
pub const MAX_MOUSE_BUTTONS: usize = glfw::MouseButton::Button8 as usize;

/// Keeps track of the current input state like pressed keys and mouse movement. Does not provide
/// functions for keys pressed this frame as layers may run at different intervals, and thus the
/// concept of frame is ambiguos. A fixed update may for example run twice for each window event
/// frame. This will cause a press for this frame to persists across multiple physics frames.
pub struct Input {
    /// If the currently pressed keys should be released when window loses focus
    release_unfocus: bool,
    rx: Receiver<InputEvent>,
    keys: [bool; MAX_KEYS],
    mouse_buttons: [bool; MAX_MOUSE_BUTTONS],
    cursor_pos: Position2D,
    scroll: Vec2,
    old_cursor_pos: Position2D,
    window_extent: Extent,
}

impl Input {
    /// Creates a new Input state handler
    pub fn new<W: Deref<Target = Window>>(window: W, events: &mut Events) -> Self {
        let (tx, rx) = flume::unbounded();
        let keys = [false; MAX_KEYS];
        let mouse_buttons = [false; MAX_MOUSE_BUTTONS];

        events.subscribe(tx);

        let window_size = window.extent();
        let cursor_pos = window.cursor_pos().into();

        Self {
            release_unfocus: true,
            rx,
            keys,
            mouse_buttons,
            cursor_pos,
            window_extent: window_size,
            old_cursor_pos: cursor_pos,
            scroll: Vec2::zero(),
        }
    }

    /// Resets the relative scroll and mouse movements and handles incoming window events. Call
    /// this each "frame".
    pub fn handle_events(&mut self) {
        self.old_cursor_pos = self.cursor_pos;
        self.scroll = Vec2::zero();

        for e in self.rx.try_iter() {
            match e {
                InputEvent::MouseButton {
                    button,
                    action,
                    mods: _,
                } => {
                    self.mouse_buttons[button as usize] =
                        action == Action::Press || action == Action::Repeat
                }
                InputEvent::CursorPos(val) => self.cursor_pos = val,
                InputEvent::Scroll(val) => self.scroll += val,
                InputEvent::Key {
                    key,
                    scancode: _,
                    action,
                    mods: _,
                } => {
                    if (key as usize) < MAX_KEYS {
                        self.keys[key as usize] =
                            action == Action::Press || action == Action::Repeat
                    }
                }
                InputEvent::Focus(false) => {
                    if self.release_unfocus {
                        self.keys = [false; MAX_KEYS];
                        self.mouse_buttons = [false; MAX_MOUSE_BUTTONS];
                    }
                }
                InputEvent::Size(extent) => self.window_extent = extent,
                _ => {}
            }
        }
    }

    /// Returns true if the given key is pressed.
    #[inline]
    pub fn key(&self, key: Key) -> bool {
        self.keys[key as usize]
    }

    /// Returns true if the given mouse button is pressed.
    #[inline]
    pub fn mouse_button(&self, button: MouseButton) -> bool {
        self.mouse_buttons[button as usize]
    }

    /// Returns the cursor positon in normalized device coordinates [-1,1]
    #[inline]
    pub fn normalized_cursor_pos(&self) -> Position2D {
        let pos = self.cursor_pos;
        Position2D::new(
            (2.0 * pos.x) / self.window_extent.width as f32 - 1.0,
            (2.0 * pos.y) / self.window_extent.height as f32 - 1.0,
        )
    }

    /// Returns the current mouse position in screen coordinates.
    #[inline]
    pub fn cursor_pos(&self) -> Position2D {
        self.cursor_pos
    }

    /// Returns the relative mouse movement in screen coordinates between this and the previous
    /// call to `on_update`. Does not take into account the time between each frame. To get the
    /// cursor velocity, divide by deltatime.
    #[inline]
    pub fn cursor_movement(&self) -> Position2D {
        self.old_cursor_pos - self.cursor_pos
    }

    /// Returns the amount scrolled this frame.
    #[inline]
    pub fn scroll(&self) -> Vec2 {
        self.scroll
    }

    /// Get a reference to the input's window extent.
    #[inline]
    pub fn window_extent(&self) -> Extent {
        self.window_extent
    }
}

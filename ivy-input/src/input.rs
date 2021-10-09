use flume::Receiver;
use glfw::{Action, Key, MouseButton, WindowEvent};
use ivy_core::Events;
use ultraviolet::Vec2;

pub const MAX_KEYS: usize = glfw::Key::Menu as usize;
pub const MAX_MOUSE_BUTTONS: usize = glfw::MouseButton::Button8 as usize;

/// Keeps track of the current input state like pressed keys and mouse movement. Does not provide
/// functions for keys pressed this frame as layers may run at different intervals, and thus the
/// concept of frame is ambiguos. A fixed update may for example run twice for each window event
/// frame. This will cause a press for this frame to persists across multiple physics frames.
pub struct Input {
    /// If the currently pressed keys should be released when window loses focus
    release_unfocus: bool,
    rx: Receiver<WindowEvent>,
    keys: [bool; MAX_KEYS],
    mouse_buttons: [bool; MAX_MOUSE_BUTTONS],
    mouse_pos: Vec2,
    scroll: Vec2,
    old_mouse_pos: Vec2,
}

impl Input {
    /// Creates a new Input state handler
    pub fn new(mouse_pos: Vec2, events: &mut Events) -> Self {
        let (tx, rx) = flume::unbounded();
        let keys = [false; MAX_KEYS];
        let mouse_buttons = [false; MAX_MOUSE_BUTTONS];

        events.subscribe(tx);

        Self {
            release_unfocus: true,
            rx,
            keys,
            mouse_buttons,
            mouse_pos,
            old_mouse_pos: mouse_pos,
            scroll: Vec2::zero(),
        }
    }

    /// Resets the relative scroll and mouse movements and handles incoming window events. Call
    /// this each frame.
    pub fn on_update(&mut self) {
        self.old_mouse_pos = self.mouse_pos;
        self.scroll = Vec2::zero();

        for e in self.rx.try_iter() {
            match e {
                WindowEvent::MouseButton(button, action, _) => {
                    self.mouse_buttons[button as usize] =
                        action == Action::Press || action == Action::Repeat
                }
                WindowEvent::CursorPos(x, y) => self.mouse_pos = Vec2::new(x as f32, y as f32),
                WindowEvent::Scroll(x, y) => self.scroll += Vec2::new(x as f32, y as f32),
                WindowEvent::Key(key, _, action, _) => {
                    self.keys[key as usize] = action == Action::Press || action == Action::Repeat
                }
                WindowEvent::Focus(false) => {
                    if self.release_unfocus {
                        self.keys = [false; MAX_KEYS];
                        self.mouse_buttons = [false; MAX_MOUSE_BUTTONS]
                    }
                }
                _ => {}
            }
        }
    }

    /// Returns true if the given key is pressed.
    pub fn key(&self, key: Key) -> bool {
        self.keys[key as usize]
    }

    /// Returns true if the given mouse button is pressed.
    pub fn mouse_button(&self, button: MouseButton) -> bool {
        self.mouse_buttons[button as usize]
    }

    /// Returns the current mouse position in screen coordinates.
    pub fn mouse_pos(&self) -> Vec2 {
        self.mouse_pos
    }

    /// Returns the relative mouse movement in screen coordinates between this and the previous
    /// call to `on_update`. Does not take into account the time between each frame. To get the
    /// cursor velocity, divide by deltatime.
    pub fn rel_mouse_pos(&self) -> Vec2 {
        self.old_mouse_pos - self.mouse_pos
    }

    /// Returns the amount scrolled this frame.
    pub fn scroll(&self) -> Vec2 {
        self.scroll
    }
}

use std::{cell::RefCell, convert::identity, ops::Deref, rc::Rc};

use flax::World;
use ivy_assets::AssetCache;
use ivy_core::{
    app::TickEvent, components::request_capture_mouse, layer::events::EventRegisterContext,
    profiling::profile_function, Layer, WorldExt,
};
use ivy_input::types::InputEvent;
use ivy_wgpu::{
    components::{main_window, window},
    driver::WindowHandle,
    events::{ApplicationReady, ResizedEvent},
};
use violet::{
    core::{declare_atom, ScopeRef, Widget},
    glam::vec2,
    wgpu::app::AppInstance,
};

use crate::{components::on_input_event, SharedUiInstance};

pub type Action = Box<dyn Send + Sync + FnOnce(&mut World, &AssetCache) -> anyhow::Result<()>>;

#[derive(Clone, Debug)]
pub struct ActionSender {
    tx: flume::Sender<Action>,
}

impl ActionSender {
    /// Invokes an action on the world after UI
    pub fn invoke(
        &self,
        action: impl 'static + Send + Sync + FnOnce(&mut World, &AssetCache) -> anyhow::Result<()>,
    ) {
        self.tx.send(Box::new(action)).expect("channel closed");
    }
}

declare_atom! {
    pub action_sender: ActionSender,
}

pub struct UiInputLayer {
    instance: Rc<RefCell<AppInstance>>,
    window: Option<WindowHandle>,
}

impl UiInputLayer {
    pub fn new(root: impl Widget) -> Self {
        let instance = AppInstance::new(root, false);
        let instance = Rc::new(RefCell::new(instance));

        Self {
            instance,
            window: None,
        }
    }

    fn on_ready(&mut self, engine_world: &mut World, _: &AssetCache) -> anyhow::Result<()> {
        let main_window = engine_world.by_tag(main_window());

        if let Some(main_window) = main_window {
            self.window = Some(main_window.get(window())?.clone());
        }

        Ok(())
    }

    fn on_input_event(
        &mut self,
        engine_world: &mut World,
        assets: &AssetCache,
        event: &InputEvent,
    ) -> anyhow::Result<bool> {
        profile_function!();
        let instance = &mut *self.instance.deref().borrow_mut();

        instance
            .input_state
            .update_external_focus(&mut instance.frame);

        // TODO: modifiers changed
        let mut captured = match event {
            InputEvent::Keyboard(keyboard_input) => instance.input_state.on_keyboard_input(
                &mut instance.frame,
                keyboard_input.key.clone(),
                keyboard_input.state,
                keyboard_input.text.clone(),
            ),
            InputEvent::ModifiersChanged(modifiers) => {
                instance.input_state.on_modifiers_change(modifiers.state());
                instance.input_state.focused().is_some()
            }
            InputEvent::Scroll(scroll_motion) => instance
                .input_state
                .on_scroll(&mut instance.frame, scroll_motion.delta),
            InputEvent::MouseButton(mouse_input) => instance.input_state.on_mouse_input(
                &mut instance.frame,
                mouse_input.state,
                mouse_input.button,
            ),
            InputEvent::CursorMoved(cursor_moved) => instance.input_state.on_cursor_move(
                &mut instance.frame,
                vec2(
                    cursor_moved.absolute_position.x,
                    cursor_moved.absolute_position.y,
                ),
            ),
            InputEvent::CursorDelta(_) => false,
            InputEvent::CursorLeft => false,
            InputEvent::CursorEntered => false,
            InputEvent::Focus(_) => false,
        };

        if let Some(focused) = instance.input_state.get_focused(instance.frame.world()) {
            let capture_mouse = focused
                .get_copy(request_capture_mouse())
                .is_ok_and(identity);

            if let Some(window) = &self.window {
                window.set_cursor_lock(capture_mouse);
            }

            if let Ok(mut handler) = focused.get_mut(on_input_event()) {
                handler(
                    &ScopeRef::new(&instance.frame, focused),
                    engine_world,
                    assets,
                    event,
                )?;

                captured = true;
            }
        }

        captured = true;
        Ok(captured)
    }

    fn on_resized(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        event: &ResizedEvent,
    ) -> anyhow::Result<()> {
        let mut instance = self.instance.deref().borrow_mut();

        instance.on_resize(event.physical_size);
        Ok(())
    }

    /// Now be careful with this one, alright?
    pub fn instance(&self) -> &SharedUiInstance {
        &self.instance
    }
}

impl Layer for UiInputLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, assets, _: &ApplicationReady| this.on_ready(world, assets));

        events.intercept(|this, world, assets, event: &InputEvent| {
            this.on_input_event(world, assets, event)
        });

        events.subscribe(|this, world, assets, event: &ResizedEvent| {
            this.on_resized(world, assets, event)
        });

        Ok(())
    }
}

pub struct UiUpdateLayer {
    instance: Rc<RefCell<AppInstance>>,
    pending_actions: flume::Receiver<Action>,
}

impl UiUpdateLayer {
    pub fn new(instance: SharedUiInstance) -> Self {
        let (tx, rx) = flume::unbounded();
        instance
            .borrow_mut()
            .frame
            .set_atom(action_sender(), ActionSender { tx });

        Self {
            instance,
            pending_actions: rx,
        }
    }

    fn on_ready(&mut self, _: &mut World, _: &AssetCache) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_tick(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        profile_function!();

        let mut instance = self.instance.deref().borrow_mut();

        instance.update();

        for action in self.pending_actions.drain() {
            action(world, assets)?;
        }

        Ok(())
    }

    fn on_resized(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        event: &ResizedEvent,
    ) -> anyhow::Result<()> {
        let mut instance = self.instance.deref().borrow_mut();

        instance.on_resize(event.physical_size);
        Ok(())
    }

    /// Now be careful with this one, alright?
    pub fn instance(&self) -> &SharedUiInstance {
        &self.instance
    }
}

impl Layer for UiUpdateLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, assets, _: &ApplicationReady| this.on_ready(world, assets));

        events.subscribe(|this, world, assets, _: &TickEvent| this.on_tick(world, assets));

        events.subscribe(|this, world, assets, event: &ResizedEvent| {
            this.on_resized(world, assets, event)
        });

        Ok(())
    }
}

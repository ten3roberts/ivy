use std::{cell::RefCell, ops::Deref, rc::Rc};

use flax::World;
use ivy_assets::AssetCache;
use ivy_core::{
    app::TickEvent, layer::events::EventRegisterContext, profiling::profile_function, Layer,
};
use ivy_input::types::InputEvent;
use ivy_wgpu::events::{ApplicationReady, ResizedEvent};
use violet::{core::Widget, glam::vec2, wgpu::app::AppInstance};

use crate::SharedUiInstance;

pub struct UiLayer {
    instance: Rc<RefCell<AppInstance>>,
}

impl UiLayer {
    pub fn new(root: impl Widget) -> Self {
        let instance = Rc::new(RefCell::new(AppInstance::new(root)));

        Self { instance }
    }

    fn on_ready(&mut self, _: &mut World, _: &mut AssetCache) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_input_event(
        &mut self,
        _: &mut World,
        _: &mut AssetCache,
        event: &InputEvent,
    ) -> anyhow::Result<bool> {
        profile_function!();
        let instance = &mut *self.instance.deref().borrow_mut();

        // TODO: modifiers changed
        let captured = match event {
            InputEvent::Keyboard(keyboard_input) => instance.input_state.on_keyboard_input(
                &mut instance.frame,
                keyboard_input.key.clone(),
                keyboard_input.state,
                None,
            ),
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

        Ok(captured)
    }

    fn on_tick(&mut self, _: &mut World, _: &mut AssetCache) -> anyhow::Result<()> {
        profile_function!();

        let mut instance = self.instance.deref().borrow_mut();

        instance.update();
        Ok(())
    }

    fn on_resized(
        &mut self,
        _: &mut World,
        _: &mut AssetCache,
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

impl Layer for UiLayer {
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
            tracing::info!(?event);
            dbg!(this.on_input_event(world, assets, event))
        });

        events.subscribe(|this, world, assets, _: &TickEvent| this.on_tick(world, assets));

        events.subscribe(|this, world, assets, event: &ResizedEvent| {
            this.on_resized(world, assets, event)
        });

        Ok(())
    }
}

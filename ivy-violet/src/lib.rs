use anyhow::Context;
use flax::World;
use ivy_assets::AssetCache;
use ivy_core::{app::TickEvent, profiling::profile_function, Layer};
use ivy_input::types::InputEvent;
use ivy_wgpu::events::{ApplicationReady, ResizedEvent};
use violet::{core::Widget, glam::vec2, wgpu::app::AppInstance};

pub struct UILayer {
    root: Option<Box<dyn Widget>>,
    instance: Option<AppInstance>,
}

impl UILayer {
    pub fn new(root: impl 'static + Widget) -> Self {
        Self {
            root: Some(Box::new(root)),
            instance: None,
        }
    }

    fn on_ready(&mut self, _: &mut World, _: &mut AssetCache) -> anyhow::Result<()> {
        let instance = AppInstance::new(self.root.take().unwrap());

        self.instance = Some(instance);

        Ok(())
    }

    fn on_input_event(
        &mut self,
        _: &mut World,
        _: &mut AssetCache,
        event: &InputEvent,
    ) -> anyhow::Result<()> {
        profile_function!();
        tracing::info!(?event, "ui input");
        let instance = self.instance.as_mut().context("instance not ready")?;

        // todo: modifiers changed
        match event {
            InputEvent::Keyboard(keyboard_input) => {
                instance.input_state.on_keyboard_input(
                    &mut instance.frame,
                    keyboard_input.key.clone(),
                    keyboard_input.state,
                    None,
                );
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
            InputEvent::CursorDelta(_) => {}
            InputEvent::CursorLeft => {}
            InputEvent::CursorEntered => {}
            InputEvent::Focus(_) => {}
        }

        Ok(())
    }

    fn on_tick(&mut self, _: &mut World, _: &mut AssetCache) -> anyhow::Result<()> {
        profile_function!();

        let instance = self.instance.as_mut().context("instance not ready")?;
        instance.update();
        Ok(())
    }

    fn on_resized(
        &mut self,
        _: &mut World,
        _: &mut AssetCache,
        event: &ResizedEvent,
    ) -> anyhow::Result<()> {
        let instance = self.instance.as_mut().context("instance not ready")?;
        instance.on_resize(event.physical_size);
        Ok(())
    }
}

impl Layer for UILayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: ivy_core::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, assets, _: &ApplicationReady| this.on_ready(world, assets));

        events.subscribe(|this, world, assets, event: &InputEvent| {
            this.on_input_event(world, assets, event)
        });

        events.subscribe(|this, world, assets, _: &TickEvent| this.on_tick(world, assets));

        events.subscribe(|this, world, assets, event: &ResizedEvent| {
            this.on_resized(world, assets, event)
        });

        Ok(())
    }
}

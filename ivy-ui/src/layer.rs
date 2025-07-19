use std::convert::identity;

use flax::World;
use ivy_assets::{stored::DynamicStore, AssetCache};
use ivy_core::{
    app::TickEvent,
    components::{engine, request_capture_mouse},
    layer::events::EventRegisterContext,
    profiling::profile_function,
    Layer, WorldExt,
};
use ivy_input::types::InputEvent;
use ivy_wgpu::{
    components::{main_window, window},
    driver::WindowHandle,
    events::{ApplicationReady, ResizedEvent},
};
use violet::{
    core::{declare_atom, style::StylesheetOptions, widget::col, ScopeRef},
    glam::vec2,
    wgpu::{app::AppInstance, AppBuilder},
};

use crate::{
    components::{on_input_event, ui_instance},
    screens::{screen_state, ScreenStack, ScreenState},
};

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

pub struct UiLayer {
    instance: Option<AppInstance>,
    window: Option<WindowHandle>,
    capture_all_input: bool,
    screens: ScreenState,
}

impl Default for UiLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl UiLayer {
    pub fn new() -> Self {
        let screens = ScreenState::new();

        let instance = AppBuilder::new()
            .with_font(violet::lucide::font_source())
            .with_stylesheet(
                StylesheetOptions::new()
                    .with_icons(violet::lucide::icon_set())
                    .build(),
            )
            .build(col(ScreenStack::new(screens.clone())).with_contain_margins(true));

        Self {
            screens,
            instance: Some(instance),
            window: None,
            capture_all_input: false,
        }
    }

    /// Capture all input events instead of feeding forward to lower layers
    pub fn with_capture_all_input(mut self, capture_all_input: bool) -> Self {
        self.capture_all_input = capture_all_input;
        self
    }

    fn on_ready(&mut self, engine_world: &mut World, _: &mut DynamicStore) -> anyhow::Result<()> {
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
        store: &mut DynamicStore,
        event: &InputEvent,
    ) -> anyhow::Result<bool> {
        profile_function!();
        let instance = store.get_mut(&*engine_world.get(engine(), ui_instance())?);

        instance.input_state.update_external_focus(&instance.frame);

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

            *engine_world.get_mut(engine(), request_capture_mouse())? = capture_mouse;

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

        captured |= self.capture_all_input;
        Ok(captured)
    }

    fn on_resized(
        &mut self,
        engine_world: &mut World,
        _: &AssetCache,
        store: &mut DynamicStore,
        event: &ResizedEvent,
    ) -> anyhow::Result<()> {
        let instance = store.get_mut(&*engine_world.get(engine(), ui_instance())?);

        instance.on_resize(event.physical_size);
        Ok(())
    }
}

impl Layer for UiLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        store: &mut DynamicStore,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        let instance_handle = store.insert(self.instance.take().expect("on_ready called twice"));

        world.set(engine(), ui_instance(), instance_handle)?;
        world.set(engine(), screen_state(), self.screens.clone())?;

        events.subscribe(|this, ctx, _: &ApplicationReady| this.on_ready(ctx.world, ctx.store));

        events.intercept(|this, ctx, event: &InputEvent| {
            this.on_input_event(ctx.world, ctx.assets, ctx.store, event)
        });

        events.subscribe(|this, ctx, event: &ResizedEvent| {
            this.on_resized(ctx.world, ctx.assets, ctx.store, event)
        });

        Ok(())
    }
}

pub struct UiUpdateLayer {
    pending_actions_rx: flume::Receiver<Action>,
    pending_actions_tx: flume::Sender<Action>,
}

impl UiUpdateLayer {
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();

        Self {
            pending_actions_rx: rx,
            pending_actions_tx: tx,
        }
    }

    fn on_ready(&mut self, world: &mut World, store: &mut DynamicStore) -> anyhow::Result<()> {
        let instance = store.get_mut(&*world.get(engine(), ui_instance())?);

        instance.frame.set_atom(
            action_sender(),
            ActionSender {
                tx: self.pending_actions_tx.clone(),
            },
        );

        Ok(())
    }

    fn on_tick(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
    ) -> anyhow::Result<()> {
        profile_function!();

        let instance = store.get_mut(&*world.get(engine(), ui_instance())?);

        instance.update();

        for action in self.pending_actions_rx.drain() {
            action(world, assets)?;
        }

        Ok(())
    }

    fn on_resized(
        &mut self,
        world: &mut World,
        store: &mut DynamicStore,
        event: &ResizedEvent,
    ) -> anyhow::Result<()> {
        let instance = store.get_mut(&*world.get(engine(), ui_instance())?);

        instance.on_resize(event.physical_size);
        Ok(())
    }
}

impl Default for UiUpdateLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Layer for UiUpdateLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, ctx, _: &ApplicationReady| this.on_ready(ctx.world, ctx.store));

        events.subscribe(|this, ctx, _: &TickEvent| this.on_tick(ctx.world, ctx.assets, ctx.store));

        events.subscribe(|this, ctx, event: &ResizedEvent| {
            this.on_resized(ctx.world, ctx.store, event)
        });

        Ok(())
    }
}

use flax::{
    fetch::{entity_refs, EntityRefs},
    Mutable, Query,
};
use ivy_base::{app::TickEvent, Layer};

use crate::{
    components::input_state,
    types::{CursorLeft, CursorMoved, KeyboardInput, MouseInput},
    InputEvent, InputState,
};

pub struct InputLayer {
    query: Query<(EntityRefs, Mutable<InputState>)>,
    last_cursor_pos: Option<glam::Vec2>,
}

impl InputLayer {
    pub fn new() -> Self {
        Self {
            query: Query::new((entity_refs(), input_state().as_mut())),
            last_cursor_pos: None,
        }
    }

    fn handle_event(&mut self, world: &mut flax::World, event: &InputEvent) {
        self.query.borrow(world).for_each(|(_, state)| {
            state.apply(event);
        });
    }

    fn update(&mut self, world: &mut flax::World) -> anyhow::Result<()> {
        self.query
            .borrow(world)
            .try_for_each(|(entity, state)| state.update(&entity))
    }
}

impl Default for InputLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Layer for InputLayer {
    fn register(
        &mut self,
        _: &mut flax::World,
        _: &ivy_assets::AssetCache,
        mut events: ivy_base::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, _, event: &KeyboardInput| -> Result<_, _> {
            this.handle_event(world, &InputEvent::Key(event.clone()));
            Ok(())
        });

        events.subscribe(|this, world, _, event: &MouseInput| -> Result<_, _> {
            this.handle_event(world, &InputEvent::MouseButton(event.clone()));
            Ok(())
        });

        events.subscribe(|this, world, _, event: &CursorMoved| {
            if let Some(last_cursor_pos) = this.last_cursor_pos {
                let delta = event.position - last_cursor_pos;
                this.handle_event(world, &InputEvent::CursorMoved(delta));
            }

            this.last_cursor_pos = Some(event.position);

            Ok(())
        });

        events.subscribe(|this, _, _, event: &CursorLeft| {
            this.last_cursor_pos = None;
            Ok(())
        });

        events.subscribe(|this, world, _, _: &TickEvent| -> Result<_, _> { this.update(world) });

        Ok(())
    }
}

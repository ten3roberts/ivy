use flax::{
    fetch::{entity_refs, EntityRefs},
    Mutable, Query,
};
use ivy_core::{app::TickEvent, Layer};

use crate::{components::input_state, InputEvent, InputState};

pub struct InputLayer {
    query: Query<(EntityRefs, Mutable<InputState>)>,
}

impl InputLayer {
    pub fn new() -> Self {
        Self {
            query: Query::new((entity_refs(), input_state().as_mut())),
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
        mut events: ivy_core::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, _, event: &InputEvent| -> Result<_, _> {
            this.handle_event(world, event);
            Ok(())
        });

        events.subscribe(|this, world, _, _: &TickEvent| -> Result<_, _> { this.update(world) });

        Ok(())
    }
}

use flax::{
    fetch::{entity_refs, EntityRefs},
    CommandBuffer, ComponentMut, Query,
};
use glam::Vec2;
use ivy_core::{app::TickEvent, components::engine, Layer};

use crate::{
    components::{cursor_position, input_state},
    Action, CursorPositionBinding, InputEvent, InputState,
};

pub struct InputLayer {
    query: Query<(EntityRefs, ComponentMut<InputState>)>,
}

impl InputLayer {
    pub fn new() -> Self {
        Self {
            query: Query::new((entity_refs(), input_state().as_mut())),
        }
    }

    fn handle_event(&mut self, world: &mut flax::World, event: &InputEvent) -> anyhow::Result<()> {
        self.query.borrow(world).for_each(|(_, state)| {
            state.apply(event);
        });

        Ok(())
    }

    fn update(&mut self, world: &mut flax::World, cmd: &mut CommandBuffer) -> anyhow::Result<()> {
        self.query
            .borrow(world)
            .try_for_each(|(entity, state)| state.update(&entity, cmd))?;

        cmd.apply(world)?;

        Ok(())
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
        world: &mut flax::World,
        _: &ivy_assets::AssetCache,
        mut events: ivy_core::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        world.set(
            engine(),
            input_state(),
            InputState::new().with_action(
                cursor_position(),
                Action::new().with_binding(CursorPositionBinding::new(true)),
            ),
        )?;

        events.subscribe(|this, ctx, event: &InputEvent| -> Result<_, _> {
            this.handle_event(ctx.world, event)
        });

        let mut cmd = CommandBuffer::new();
        events.subscribe(move |this, ctx, _: &TickEvent| -> Result<_, _> {
            this.update(ctx.world, &mut cmd)
        });

        Ok(())
    }
}

use anyhow::Context;
use flax::{BoxedSystem, System, World};

use crate::AsyncCommandBuffer;

pub fn apply_async_commandbuffers(cmd: AsyncCommandBuffer) -> BoxedSystem {
    System::builder()
        .with_world_mut()
        .build(move |world: &mut World| {
            cmd.lock()
                .apply(world)
                .context("Failed to apply async commandbuffer")
        })
        .boxed()
}

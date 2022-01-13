use hecs::{Component, Entity, EntityBuilderClone, World};
use hecs_hierarchy::DeferredTreeBuilder;
use hecs_schedule::{CommandBuffer, GenericWorld, SubWorldRef};

pub trait Template: Component {
    fn root(&self) -> &EntityBuilderClone;
    fn root_mut(&mut self) -> &mut EntityBuilderClone;
    fn build(&self, world: &mut World, extra: EntityBuilderClone) -> Entity;
    fn build_cmd(
        &self,
        world: &SubWorldRef<()>,
        cmd: &mut CommandBuffer,
        extra: EntityBuilderClone,
    ) -> Entity;
}

impl Template for EntityBuilderClone {
    fn root(&self) -> &EntityBuilderClone {
        &self
    }

    fn root_mut(&mut self) -> &mut EntityBuilderClone {
        self
    }

    fn build(&self, world: &mut World, extra: EntityBuilderClone) -> Entity {
        let mut builder = self.clone();
        builder.add_bundle(&extra.build());
        world.spawn(&builder.build())
    }

    fn build_cmd(
        &self,
        world: &SubWorldRef<()>,
        cmd: &mut CommandBuffer,
        extra: EntityBuilderClone,
    ) -> Entity {
        let mut builder = self.clone();
        builder.add_bundle(&extra.build());
        let entity = world.reserve();
        cmd.insert(entity, &builder.build());
        entity
    }
}

impl<T: Component + Clone> Template for DeferredTreeBuilder<T> {
    fn root(&self) -> &EntityBuilderClone {
        self.builder()
    }

    fn root_mut(&mut self) -> &mut EntityBuilderClone {
        self.builder_mut()
    }

    fn build(&self, world: &mut World, extra: EntityBuilderClone) -> Entity {
        let mut builder = self.clone();

        builder.builder_mut().add_bundle(&extra.build());
        builder.spawn(world)
    }

    fn build_cmd(
        &self,
        world: &SubWorldRef<()>,
        cmd: &mut CommandBuffer,
        extra: EntityBuilderClone,
    ) -> Entity {
        let mut builder = self.clone();

        builder.builder_mut().add_bundle(&extra.build());
        builder.spawn_deferred(world, cmd)
    }
}

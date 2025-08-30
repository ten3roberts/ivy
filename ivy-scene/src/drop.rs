use flax::{component, Entity};
use glam::Vec2;
use ivy_ui::violet::core::{
    style::SizeExt,
    widget::{drop_target, Stack},
    Scope, Widget,
};

pub struct WorldDropArea {
    scene_id: Entity,
}

impl WorldDropArea {
    pub fn new(scene_id: Entity) -> Self {
        Self { scene_id }
    }
}

impl Widget for WorldDropArea {
    fn mount(self, scope: &mut Scope<'_>) {
        scope
            .set_default(drop_target())
            .set(world_drop_area(), self.scene_id);

        Stack::new(()).with_maximize(Vec2::ONE).mount(scope);
    }
}

component! {
    pub world_drop_area: Entity,
}

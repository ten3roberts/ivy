use flax::component;
use glam::Vec2;
use ivy_core::palette::Srgba;
use ivy_ui::violet::core::{
    Scope, Widget,
    style::SizeExt,
    widget::{Stack, drop_target},
};

pub struct WorldDropArea {}

impl Widget for WorldDropArea {
    fn mount(self, scope: &mut Scope<'_>) {
        scope
            .set_default(drop_target())
            .set_default(world_drop_area());

        Stack::new(()).with_maximize(Vec2::ONE).mount(scope);
    }
}

component! {
    pub world_drop_area: (),
}

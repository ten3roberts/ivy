use flax::component::ComponentValue;
use flax::{Component, Entity};
use futures::{FutureExt, channel::oneshot};
use glam::Vec2;
use itertools::Itertools;
use ivy_scene::editor::registry;
use ivy_ui::streamed::{StreamedUiExt, streamed_tx};
use ivy_ui::violet;
use ivy_ui::violet::core::components::{local_position, transform_origin, translation, visible};
use ivy_ui::violet::core::style::base_colors::{STONE_700, STONE_800, ZINC_700, ZINC_800};
use ivy_ui::violet::core::style::{surface_primary, surface_success};
use ivy_ui::violet::core::tweens;
use ivy_ui::violet::core::widget::Stack;
use tween::{Tween, TweenValue, Tweener};
use violet::core::{
    Widget,
    style::{SizeExt, StyleExt, spacing_large, surface_tertiary},
    unit::Unit,
    widget::{Collapsible, CollapsibleStyle, FutureWidget, bold, card, col, label},
};

use registry::EDITABLE_REGISTRY;

/// Allows editing components of the entity
pub struct EntityComponentEditor {
    entity: Entity,
}

impl EntityComponentEditor {
    pub fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

impl Widget for EntityComponentEditor {
    fn mount(self, scope: &mut violet::core::Scope<'_>) {
        let streamed = scope.get_context_cloned(streamed_tx());

        let (tx, rx) = oneshot::channel();

        scope.apply(move |world| {
            let entity = world.entity(self.entity)?;

            let components = entity
                .components()
                .sorted_by_key(|desc| (!EDITABLE_REGISTRY.contains(desc.type_id()), desc.name()));

            let editors = components
                .filter_map(|desc| {
                    let editor = EDITABLE_REGISTRY.create_component_editor(
                        entity,
                        desc,
                        streamed.clone(),
                    )?;

                    Some(
                        card(
                            Collapsible::label(desc.name(), editor)
                                .with_min_size(Unit::px2(300.0, 0.0))
                                .with_max_size(Unit::px2(300.0, 200.0)),
                        )
                        .with_background(surface_tertiary()),
                    )
                })
                .collect_vec();

            let _ = tx.send(editors);
            Ok(())
        });

        FutureWidget::new(rx.map(move |v| v.ok().map(|editors| col(editors)))).mount(scope)
    }
}

pub struct Animate<W, T, A> {
    pub content: W,
    pub component: Component<T>,
    pub tween: Tweener<T, f32, A>,
}

impl<W, T, A> Animate<W, T, A> {
    pub fn new(content: W, component: Component<T>, tween: Tweener<T, f32, A>) -> Self {
        Self {
            content,
            component,
            tween,
        }
    }
}

impl<W, T, A> Widget for Animate<W, T, A>
where
    W: Widget,
    T: ComponentValue + TweenValue,
    A: 'static + Send + Sync + Tween<T>,
{
    fn mount(self, scope: &mut violet::core::Scope<'_>) {
        Stack::new(self.content).mount(scope);

        let start: Vec2 = Vec2::X * 100.0;
        scope
            .set(visible(), true)
            .set(translation(), start)
            .set_default(transform_origin())
            .set_default(violet::core::components::rotation())
            .set_default(tweens::tweens());

        scope.add_tween(self.component, self.tween);
    }
}

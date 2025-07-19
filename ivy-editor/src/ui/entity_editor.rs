use flax::Entity;
use futures::{FutureExt, channel::oneshot};
use itertools::Itertools;
use ivy_editable::registry;
use ivy_ui::streamed::{StreamedUiExt, streamed_tx};
use ivy_ui::violet::{self};
use violet::core::{
    Widget,
    widget::{FutureWidget, col},
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
        let _streamed = scope.get_context_cloned(streamed_tx());

        let (tx, rx) = oneshot::channel();

        scope.apply(move |world| {
            let entity = world.entity(self.entity)?;

            let components = entity
                .components()
                .sorted_by_key(|desc| (!EDITABLE_REGISTRY.contains(desc.type_id()), desc.name()));

            let editors = components
                .filter_map(|_desc| {
                    None as Option<Box<dyn Widget + Send>>
                    // let editor = EDITABLE_REGISTRY.create_component_editor(
                    //     entity,
                    //     desc,
                    //     streamed.clone(),
                    // )?;

                    // Some(
                    //     card(
                    //         Collapsible::label(desc.name(), editor)
                    //             .with_min_size(Unit::px2(300.0, 0.0))
                    //             .with_max_size(Unit::px2(300.0, 200.0)),
                    //     )
                    //     .with_background(surface_tertiary()),
                    // )
                })
                .collect_vec();

            let _ = tx.send(editors);
            Ok(())
        });

        FutureWidget::new(rx.map(move |v| v.ok().map(col))).mount(scope)
    }
}

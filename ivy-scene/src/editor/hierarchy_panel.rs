use std::collections::BTreeMap;

use flax::{
    components::{child_of, name},
    entity_ids, Dfs, Entity, FetchExt, Query,
};
use futures::StreamExt;
use itertools::Itertools;
use ivy_core::components::world_transform;
use ivy_ui::{
    streamed::StreamedUiExt,
    violet::{
        core::{
            state::StateStreamRef,
            stored::WeakHandle,
            style::{SizeExt, WidgetSizeProps},
            widget::{card, col, label, Button, Collapsible},
            Edges, Scope, Widget,
        },
        futures_signals::signal::Mutable,
    },
};

pub struct HierarchyPanel {
    size: WidgetSizeProps,
}

impl HierarchyPanel {
    pub fn new() -> Self {
        Self {
            size: Default::default(),
        }
    }
}

impl Widget for HierarchyPanel {
    fn mount(self, scope: &mut Scope<'_>) {
        let (tx, rx) = flume::unbounded();

        scope.apply(move |world| {
            let mut hierarchy: BTreeMap<Option<Entity>, Vec<EntityHandle>> = BTreeMap::new();
            let mut query = Query::new((entity_ids(), name().opt()))
                .with(world_transform())
                .with_strategy(Dfs::new(child_of));

            query
                .borrow(world)
                .traverse(&None, |(id, name), _, parent| {
                    hierarchy.entry(*parent).or_default().push(EntityHandle {
                        id,
                        name: name.cloned(),
                    });

                    Some(id)
                });

            let _ = tx.send(hierarchy);
            Ok(())
        });

        scope.spawn_stream(rx.into_stream(), |scope, hierarchy| {
            let roots = hierarchy
                .get(&None)
                .into_iter()
                .flatten()
                .map(|roots| SubtreeWidget {
                    entity: roots.clone(),
                    hierarchy: &hierarchy,
                });

            scope.detach_all();

            scope.attach(col(roots.collect_vec()).with_stretch(true));
        });
    }
}

#[derive(PartialEq, PartialOrd, Eq, Ord, Clone)]
struct EntityHandle {
    id: Entity,
    name: Option<String>,
}

struct SubtreeWidget<'a> {
    entity: EntityHandle,
    hierarchy: &'a BTreeMap<Option<Entity>, Vec<EntityHandle>>,
}

impl Widget for SubtreeWidget<'_> {
    fn mount(self, scope: &mut Scope<'_>) {
        // let children = scope.read(&self.hierarchy).borrow();

        let children = self.hierarchy.get(&Some(self.entity.id));
        let label = label(
            self.entity
                .name
                .clone()
                .unwrap_or_else(|| self.entity.id.to_string()),
        );

        if let Some(children) = children {
            let widget = Collapsible::new(
                label,
                col(children
                    .iter()
                    .map(|child| SubtreeWidget {
                        entity: child.clone(),
                        hierarchy: self.hierarchy,
                    })
                    .collect_vec())
                .with_padding(Edges::new(16.0, 0.0, 0.0, 0.0))
                .with_stretch(true),
            );

            widget.mount(scope);
        } else {
            Button::new(label).mount(scope);
        }
    }
}

impl SizeExt for HierarchyPanel {
    fn size_mut(&mut self) -> &mut WidgetSizeProps {
        &mut self.size
    }
}

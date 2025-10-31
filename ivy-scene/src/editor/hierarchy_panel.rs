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
    violet::core::{
        stored::WeakHandle,
        style::{SizeExt, StyleExt, WidgetSizeProps},
        unit::Unit,
        widget::{
            card, col, label, ButtonStyle, Collapsible, CollapsibleStyle, ScrollArea, StreamWidget,
        },
        Edges, Scope, Widget,
    },
};

pub struct HierarchyPanel {
    size: WidgetSizeProps,
}

impl Default for HierarchyPanel {
    fn default() -> Self {
        Self::new()
    }
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

        let inner = {
            let contents = rx.into_stream().map(|new_hierarchy| {
                |scope: &mut Scope<'_>| {
                    let hierarchy = scope.store(new_hierarchy);
                    let mut roots = scope
                        .read(&hierarchy)
                        .get(&None)
                        .into_iter()
                        .flatten()
                        .map(|roots| SubtreeWidget {
                            entity: roots.clone(),
                            hierarchy,
                        })
                        .collect_vec();

                    roots.sort_by(|a, b| a.entity.name.cmp(&b.entity.name));

                    col(roots).with_stretch(true).mount(scope);
                }
            });

            StreamWidget::new(contents)
        };

        card(ScrollArea::vertical(inner).with_min_size(Unit::px2(160.0, 400.0))).mount(scope)
    }
}

#[derive(PartialEq, PartialOrd, Eq, Ord, Clone)]
struct EntityHandle {
    id: Entity,
    name: Option<String>,
}

struct SubtreeWidget {
    entity: EntityHandle,
    hierarchy: WeakHandle<BTreeMap<Option<Entity>, Vec<EntityHandle>>>,
}

impl Widget for SubtreeWidget {
    fn mount(self, scope: &mut Scope<'_>) {
        // let children = scope.read(&self.hierarchy).borrow();

        let children = scope
            .read(&self.hierarchy)
            .get(&Some(self.entity.id))
            .cloned();

        let name = label(
            self.entity
                .name
                .clone()
                .unwrap_or_else(|| self.entity.id.to_string()),
        );

        let can_collapse = children
            .as_ref()
            .map(|children| !children.is_empty())
            .unwrap_or(false);

        let widget = Collapsible::new(
            name,
            col(children
                .iter()
                .flat_map(|children| children.iter())
                .map(|child| SubtreeWidget {
                    entity: child.clone(),
                    hierarchy: self.hierarchy,
                })
                .collect_vec())
            .with_padding(Edges::new(16.0, 0.0, 0.0, 0.0))
            .with_stretch(true),
        )
        .can_collapse(can_collapse)
        .with_style(CollapsibleStyle::default().with_button_style(ButtonStyle::selectable_entry()));

        widget.mount(scope);
    }
}

impl SizeExt for HierarchyPanel {
    fn size_mut(&mut self) -> &mut WidgetSizeProps {
        &mut self.size
    }
}

use crate::{apply_constraints, update_from, Result, Widget};
use hecs::Entity;
use hecs_hierarchy::Hierarchy;
use hecs_schedule::GenericWorld;
use ivy_base::{Position2D, Size2D};

/// UI component for automatically managing placing of children.
/// Immediate children of a widget with a layout will be placed automatically
/// and have their position constraints ignored.
pub struct WidgetLayout {
    kind: LayoutKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutKind {
    Horizontal,
}
impl WidgetLayout {
    pub fn new(kind: LayoutKind) -> Self {
        Self { kind }
    }

    pub fn update(
        &self,
        world: &impl GenericWorld,
        parent: Entity,
        position: Position2D,
        size: Size2D,
        depth: u32,
    ) -> Result<()> {
        let mut cursor = *position - *size;

        world
            .children::<Widget>(parent)
            .try_for_each(|child| -> Result<()> {
                apply_constraints(world, child, position, size)?;
                let mut query = world.try_query_one::<(&mut Position2D, &Size2D)>(child)?;

                let (child_pos, child_size) = query.get()?;

                *child_pos = Position2D(cursor + **child_size);
                cursor.x += child_size.x * 2.0;

                drop(query);

                update_from(world, child, depth + 1)?;
                Ok(())
            })?;

        Ok(())
    }
}

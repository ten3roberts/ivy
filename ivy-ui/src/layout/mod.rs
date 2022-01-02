use crate::{apply_constraints, update_from, Alignment, Result, Widget};
use fontdue::layout::{HorizontalAlign, VerticalAlign};
use hecs::Entity;
use hecs_hierarchy::Hierarchy;
use hecs_schedule::GenericWorld;
use ivy_base::{Position2D, Size2D};
use glam::Vec2;

/// UI component for automatically managing placing of children.
/// Immediate children of a widget with a layout will be placed automatically
/// and have their position constraints ignored.
#[records::record]
pub struct WidgetLayout {
    kind: LayoutKind,
    align: Alignment,
    spacing: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutKind {
    Horizontal,
    Vertical,
}

impl WidgetLayout {
    pub fn update(
        &self,
        world: &impl GenericWorld,
        parent: Entity,
        position: Position2D,
        size: Size2D,
        depth: u32,
        is_visible: bool,
    ) -> Result<()> {
        let mut iter = world.children::<Widget>(parent);

        let total_size: Size2D =
            iter.try_fold(Size2D::default(), |acc, child| -> Result<Size2D> {
                apply_constraints(world, child, position, size, is_visible)?;

                let child_size = world.try_get::<Size2D>(child)?;

                Ok(acc + *child_size)
            })?;

        let total_size = match self.kind {
            LayoutKind::Horizontal => Size2D::new(total_size.x, size.y),
            LayoutKind::Vertical => Size2D::new(0.0, total_size.y),
        };

        let x = match self.align.horizontal {
            HorizontalAlign::Left => position.x - size.x,
            HorizontalAlign::Center => position.x + total_size.x,
            HorizontalAlign::Right => position.x + total_size.x,
        };

        let y = match self.align.vertical {
            VerticalAlign::Top => position.y + size.y - total_size.y,
            VerticalAlign::Middle => position.y + total_size.y,
            VerticalAlign::Bottom => position.y - size.y + total_size.y,
        };

        let mut iter = world.children::<Widget>(parent);
        match self.kind {
            LayoutKind::Horizontal => {
                let mut cursor = Position2D::new(x, y);
                iter.try_for_each(|child| -> Result<()> {
                    apply_constraints(world, child, position, size, is_visible)?;
                    let mut query = world.try_query_one::<(&mut Position2D, &Size2D)>(child)?;

                    let (child_pos, child_size) = query.get()?;

                    *child_pos = cursor; //+ Position2D(**child_size);
                    cursor.x += child_size.x * 2.0 + self.spacing.x;

                    drop(query);

                    update_from(world, child, depth + 1)?;
                    Ok(())
                })?
            }

            LayoutKind::Vertical => {
                let offset_x = match self.align.horizontal {
                    HorizontalAlign::Left => 1.0,
                    HorizontalAlign::Center => 0.0,
                    HorizontalAlign::Right => -1.0,
                };

                let mut cursor = Position2D::new(x, y);
                iter.try_for_each(|child| -> Result<()> {
                    apply_constraints(world, child, position, size, is_visible)?;
                    let mut query = world.try_query_one::<(&mut Position2D, &Size2D)>(child)?;

                    let (child_pos, child_size) = query.get()?;

                    *child_pos = cursor + Position2D::new(child_size.x * offset_x, 0.0);
                    cursor.y -= child_size.y * 2.0 + self.spacing.y;

                    drop(query);

                    update_from(world, child, depth + 1)?;
                    Ok(())
                })?
            }
        }

        Ok(())
    }
}

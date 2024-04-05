use crate::{apply_constraints, children, update_from, Alignment, Result};
use flax::{EntityRef, World};
use fontdue::layout::{HorizontalAlign, VerticalAlign};
use glam::Vec2;

/// UI component for automatically managing placing of children.
/// Immediate children of a widget with a layout will be placed automatically
/// and have their position constraints ignored.
#[derive(Clone, Copy)]
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
    pub fn new(kind: LayoutKind, align: Alignment, spacing: Vec2) -> Self {
        Self {
            kind,
            align,
            spacing,
        }
    }

    pub fn update(
        &self,
        world: &World,
        parent: &EntityRef,
        position: Vec2,
        size: Vec2,
        depth: u32,
        is_visible: bool,
    ) -> Result<()> {
        let total_size: Vec2 = parent
            .get(children())
            .ok()
            .iter()
            .flat_map(|v| v.iter())
            .map(|&v| world.entity(v).unwrap())
            .try_fold(Vec2::default(), |acc, child| -> Result<Vec2> {
                apply_constraints(world, &child, position, size, is_visible)?;

                let child_size = child
                    .get(ivy_base::components::size())
                    .expect("Missing size");
                // let child_size = world.try_get::<Vec2>(child)?;

                Ok(acc + *child_size)
            })?;

        let total_size = match self.kind {
            LayoutKind::Horizontal => Vec2::new(total_size.x, size.y),
            LayoutKind::Vertical => Vec2::new(0.0, total_size.y),
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

        let c = parent.get(children()).ok();
        let mut iter = c
            .iter()
            .flat_map(|v| v.iter())
            .map(|&v| world.entity(v).unwrap());

        match self.kind {
            LayoutKind::Horizontal => {
                let mut cursor = Vec2::new(x, y);
                iter.try_for_each(|child| -> Result<()> {
                    apply_constraints(world, &child, position, size, is_visible)?;
                    {
                        let mut child_position =
                            child.get_mut(ivy_base::components::position()).unwrap();
                        let child_size = child.get_copy(ivy_base::components::size()).unwrap();

                        *child_position = cursor.extend(0.0); //+ Position2D(**child_size);
                        cursor.x += child_size.x * 2.0 + self.spacing.x;
                    }

                    update_from(world, &child, depth + 1)?;
                    Ok(())
                })?
            }

            LayoutKind::Vertical => {
                let offset_x = match self.align.horizontal {
                    HorizontalAlign::Left => 1.0,
                    HorizontalAlign::Center => 0.0,
                    HorizontalAlign::Right => -1.0,
                };

                let mut cursor = Vec2::new(x, y);
                iter.try_for_each(|child| -> Result<()> {
                    apply_constraints(world, &child, position, size, is_visible)?;
                    {
                        let mut child_position =
                            child.get_mut(ivy_base::components::position()).unwrap();
                        let child_size = child.get_copy(ivy_base::components::size()).unwrap();

                        // let mut query = world.try_query_one::<(&mut Position2D, &Size2D)>(child)?;

                        // let (child_pos, child_size) = query.get()?;

                        *child_position =
                            (cursor + Vec2::new(child_size.x * offset_x, 0.0)).extend(0.0);
                        cursor.y -= child_size.y * 2.0 + self.spacing.y;
                    }

                    // drop(query);

                    update_from(world, &child, depth + 1)?;
                    Ok(())
                })?
            }
        }

        Ok(())
    }
}

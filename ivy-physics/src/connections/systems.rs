use crate::Result;
use hecs::*;
use hecs_hierarchy::*;
use ivy_base::{Color, Gizmos, Position, TransformQuery, TransformQueryMut};

use super::*;

pub fn update_connections(world: &World) -> Result<()> {
    world
        .roots::<Connection>()
        .into_iter()
        .try_for_each(|root| update_subtree(world, root.0))
}

fn update_subtree(world: &World, root: Entity) -> Result<()> {
    let mut query = world.query_one::<(TransformQuery, RbQuery)>(root)?;

    if let Some((parent_trans, parent_rb)) = query.get() {
        let parent_trans = parent_trans.into_owned();
        let parent_rb = parent_rb.into_owned();
        drop(query);

        // Create an effector for storing the applied effects
        let mut dummy_effector = Effector::new();

        world
            .children::<Connection>(root)
            .try_for_each(|child| -> Result<_> {
                let mut query = world.query_one::<(
                    &PositionOffset,
                    &RotationOffset,
                    TransformQueryMut,
                    RbQueryMut,
                    &ConnectionKind,
                    &mut Effector,
                )>(child)?;

                if let Some((
                    offset_pos,
                    offset_rot,
                    child_trans,
                    child_rb,
                    connection_kind,
                    effector,
                )) = query.get()
                {
                    connection_kind.update(
                        offset_pos,
                        offset_rot,
                        child_trans,
                        child_rb,
                        &parent_trans,
                        &parent_rb,
                        effector,
                        &mut dummy_effector,
                    );
                }
                drop(query);
                update_subtree(world, child)?;

                Ok(())
            })?;

        let mut effector = world.get_mut::<Effector>(root)?;
        *effector += dummy_effector;

        Ok(())
    } else {
        Ok(())
    }
}

/// Recursively draw the connection tree using gizmos
pub fn draw_connections(world: &World, gizmos: &mut Gizmos) -> Result<()> {
    world
        .roots::<Connection>()
        .into_iter()
        .try_for_each(|root| draw_subtree(world, root.0, gizmos))
}

fn draw_subtree(world: &World, root: Entity, gizmos: &mut Gizmos) -> Result<()> {
    let parent_pos = world.get::<Position>(root)?;

    world
        .children::<Connection>(root)
        .try_for_each(|child| -> Result<()> {
            let mut query = world.query_one::<(&Position, &ConnectionKind)>(child)?;
            let (pos, kind) = query
                .get()
                .expect("Failed to execute query in draw_connections");

            let color = match kind {
                ConnectionKind::Rigid => Color::green(),
                ConnectionKind::Spring {
                    strength: _,
                    dampening: _,
                } => Color::red(),
            };

            gizmos.push(ivy_base::Gizmo::Line {
                origin: **parent_pos,
                color,
                dir: *(*pos - *parent_pos),
                radius: 0.02,
                corner_radius: 1.0,
            });

            draw_subtree(world, child, gizmos)
        })
}

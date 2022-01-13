use crate::{Effector, Result};
use hecs::*;
use hecs_hierarchy::*;
use hecs_schedule::{GenericWorld, SubWorld};
use ivy_base::{Color, Connection, Gizmos, Position, TransformQuery};

use super::*;

pub fn update_connections(
    world: SubWorld<(
        &ConnectionKind,
        &PositionOffset,
        &RotationOffset,
        &mut Effector,
        HierarchyQuery<Connection>,
        RbQueryMut,
        TransformQueryMut,
    )>,
) -> Result<()> {
    world
        .roots::<Connection>()?
        .into_iter()
        .try_for_each(|root| update_subtree(&world, root.0))
}

fn update_subtree(world: &impl GenericWorld, root: Entity) -> Result<()> {
    let mut query = world.try_query_one::<(TransformQuery, Option<RbQuery>)>(root)?;

    if let Ok((parent_trans, rb)) = query.get() {
        let parent_trans = parent_trans.into_owned();
        let mut parent_rb = rb.map(|val| RbBundle {
            vel: *val.vel,
            mass: *val.mass,
            ang_mass: *val.ang_mass,
            ang_vel: *val.ang_vel,
            resitution: *val.resitution,
            effector: Effector::new(),
        });

        drop(query);

        world
            .children::<Connection>(root)
            .try_for_each(|child| -> Result<_> {
                let mut fixed = world
                    .try_query_one::<(&PositionOffset, &RotationOffset, TransformQueryMut)>(
                        child,
                    )?;

                let (offset_pos, offset_rot, child_trans) = fixed.get()?;

                let mut dynamic =
                    world.try_query_one::<(RbQueryMut, &ConnectionKind, &mut Effector)>(child)?;

                if let (Some(parent_rb), Ok((child_rb, connection_kind, effector))) =
                    (parent_rb.as_mut(), dynamic.get())
                {
                    update_connection(
                        connection_kind,
                        offset_pos,
                        offset_rot,
                        child_trans,
                        child_rb,
                        &parent_trans,
                        parent_rb,
                        effector,
                    );
                } else {
                    update_fixed(offset_pos, offset_rot, &parent_trans, child_trans);
                }

                drop((fixed, dynamic));
                update_subtree(world, child)?;

                Ok(())
            })?;

        if let Some(rb) = parent_rb {
            let mut effector = world.try_get_mut::<Effector>(root)?;
            *effector += rb.effector;
        }

        Ok(())
    } else {
        Ok(())
    }
}

/// Recursively draw the connection tree using gizmos
pub fn draw_connections(world: &impl GenericWorld, gizmos: &mut Gizmos) -> Result<()> {
    world
        .roots::<Connection>()?
        .into_iter()
        .try_for_each(|root| draw_subtree(world, root.0, gizmos))
}

fn draw_subtree(world: &impl GenericWorld, root: Entity, gizmos: &mut Gizmos) -> Result<()> {
    let parent_pos = world.try_get::<Position>(root)?;

    world
        .children::<Connection>(root)
        .try_for_each(|child| -> Result<()> {
            let mut query = world.try_query_one::<(&Position, &ConnectionKind)>(child)?;
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

            gizmos.draw(ivy_base::Gizmo::Line {
                origin: *parent_pos,
                color,
                dir: *(*pos - *parent_pos),
                radius: 0.02,
                corner_radius: 1.0,
            });

            draw_subtree(world, child, gizmos)
        })
}

use crate::{Effector, Result};
use hecs::Entity;
use hecs_hierarchy::*;
use hecs_schedule::*;
use ivy_base::{Color, Connection, Gizmos, Position, Static, TransformQuery};

use super::*;

pub struct UpdatedStatic;
pub fn update_connections(
    world: SubWorld<(
        &ConnectionKind,
        &PositionOffset,
        &RotationOffset,
        &mut Effector,
        &Static,
        &UpdatedStatic,
        HierarchyQuery<Connection>,
        RbQueryMut,
        TransformQueryMut,
    )>,
    mut cmd: Write<CommandBuffer>,
) -> Result<()> {
    world
        .roots::<Connection>()?
        .into_iter()
        .try_for_each(|root| update_subtree(&world, &mut *cmd, root.0))
}

fn update_subtree(world: &impl GenericWorld, cmd: &mut CommandBuffer, root: Entity) -> Result<()> {
    let mut query = world.try_query_one::<(
        TransformQuery,
        Option<RbQuery>,
        Option<&Static>,
        Option<&UpdatedStatic>,
    )>(root)?;

    if let Ok((parent_trans, rb, is_static, updated)) = query.get() {
        if is_static.is_some() && updated.is_some() {
            return Ok(());
        } else if is_static.is_some() {
            cmd.insert_one(root, UpdatedStatic);
        }

        let parent_trans = parent_trans.into_owned();
        let mut parent_rb = rb.map(|val| RbBundle {
            vel: *val.vel,
            mass: *val.mass,
            ang_mass: *val.ang_mass,
            ang_vel: *val.ang_vel,
            resitution: *val.resitution,
            effector: Effector::default(),
            friction: *val.friction,
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
                update_subtree(world, cmd, child)?;

                Ok(())
            })?;

        if let Some(rb) = parent_rb {
            let mut effector = world.try_get_mut::<Effector>(root)?;
            effector.apply_other(&rb.effector);
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

            gizmos.draw(
                ivy_base::Line {
                    origin: **parent_pos,
                    dir: *(*pos - *parent_pos),
                    radius: 0.02,
                    corner_radius: 1.0,
                },
                color,
            );

            draw_subtree(world, child, gizmos)
        })
}

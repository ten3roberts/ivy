use crate::{components::effector, Effector, Result};
use flax::{
    fetch::{entity_refs, EntityRefs},
    BoxedSystem, CommandBuffer, Dfs, EntityRef, FetchExt, Query, QueryBorrow, System, World,
};
use ivy_base::{
    connection, is_static, position_offset, rotation_offset, world_transform, Color, ColorExt, Gizmos,
};

use super::*;

// pub struct ConnectionQuery {
//     transform:Component<Mat4>,
//     rb:Component<RbQuery>,
//     is_static: Satisfied<Component<()>>)
// }

pub struct UpdatedStatic;
pub fn update_connections_system() -> BoxedSystem {
    let query = Query::new(entity_refs()).with_relation(connection);

    System::builder()
        .with_world()
        .with_cmd_mut()
        .with_query(query)
        .build(
            |world: &World, cmd: &mut CommandBuffer, mut query: QueryBorrow<EntityRefs, _>| {
                query.iter().for_each(|root| {
                    update_subtree(world, &mut *cmd, root);
                })
            },
        )
        .boxed()
}
// pub fn update_connections(world: &World, mut cmd: Write<CommandBuffer>) -> Result<()> {
//     world
//         .roots::<Connection>()?
//         .into_iter()
//         .try_for_each(|root| update_subtree(&world, &mut *cmd, root.0))
// }

fn update_subtree(world: &World, cmd: &mut CommandBuffer, parent: EntityRef) -> Result<()> {
    let query = (world_transform(), RbQuery::new().opt(), is_static().opt());
    // let mut query = world.try_query_one::<(
    //     TransformQuery,
    //     Option<RbQuery>,
    //     Option<&Static>,
    //     Option<&UpdatedStatic>,
    // )>(root)?;

    let mut query = parent.query(&query);
    let Some((&parent_trans, rb, is_static)) = query.get() else {
        return Ok(());
    };

    // if is_static.is_some() && updated.is_some() {
    //     return Ok(());
    // } else if is_static.is_some() {
    //     // cmd.set(parent, UpdatedStatic);
    // }

    let mut parent_rb = rb.map(|val| RbBundle {
        vel: *val.vel,
        mass: *val.mass,
        ang_mass: *val.ang_mass,
        ang_vel: *val.ang_vel,
        restitution: *val.restitution,
        // effector: Effector::default(),
        friction: *val.friction,
    });

    let mut parent_effector = Effector::default();

    drop(query);

    Query::new((entity_refs(), connection(parent.id())))
        .borrow(world)
        .iter()
        .try_for_each(|(child, connection)| {
            // let child = world.entity(child).unwrap();

            let query = (
                position_offset(),
                rotation_offset(),
                TransformQueryMut::new(),
            );

            // let mut fixed = world
            //     .try_query_one::<(&PositionOffset, &RotationOffset, TransformQueryMut)>(
            //         child,
            //     )?;

            {
                let mut query = child.query(&query);
                let (&offset_pos, &offset_rot, mut child_trans) = query.get().unwrap();

                let query = &(RbQueryMut::new(), effector().as_mut());

                let mut query = child.query(query);
                // let mut dynamic = world
                //     .try_query_one::<(RbQueryMut, &ConnectionKind, &mut Effector)>(child)?;

                if let (Some(parent_rb), Some((child_rb, effector))) =
                    (parent_rb.as_mut(), query.get())
                {
                    apply_connection_constraints(
                        connection,
                        offset_pos,
                        offset_rot,
                        child_trans,
                        child_rb,
                        parent_trans,
                        parent_rb,
                        effector,
                        &mut parent_effector,
                    );
                } else {
                    update_fixed(offset_pos, offset_rot, parent_trans, &mut child_trans);
                }
            }

            update_subtree(world, cmd, child)?;

            Ok(()) as crate::Result<()>
        })?;

    if let Some(rb) = parent_rb {
        parent
            .get_mut(effector())
            .unwrap()
            .apply_other(&parent_effector);
        // let mut effector = world.try_get_mut::<Effector>(parent)?;
        // effector.apply_other(&rb.effector);
    }

    Ok(())
}

/// Recursively draw the connection tree using gizmos
pub fn draw_connections(world: &World, gizmos: &mut Gizmos) -> Result<()> {
    Query::new((entity_refs(), world_transform()))
        .with_strategy(Dfs::new(connection))
        .borrow(world)
        .traverse(&Vec3::ZERO, |(entity, transform), conn, &parent_pos| {
            // let mut query = world.try_query_one::<(&Position, &ConnectionKind)>(child)?;
            // let (pos, kind) = query
            //     .get()
            //     .expect("Failed to execute query in draw_connections");

            let pos = transform.transform_point3(Vec3::ZERO);

            if let Some(kind) = conn {
                let color = match kind {
                    ConnectionKind::Rigid => Color::green(),
                    ConnectionKind::Spring {
                        strength: _,
                        dampening: _,
                    } => Color::red(),
                };

                gizmos.draw(
                    ivy_base::Line {
                        origin: parent_pos,
                        dir: (pos - parent_pos),
                        radius: 0.02,
                        corner_radius: 1.0,
                    },
                    color,
                );
            }

            pos
        });

    Ok(())
    // world
    //     .roots::<Connection>()?
    //     .into_iter()
    //     .try_for_each(|root| draw_subtree(world, root.0, gizmos))
}

// fn draw_subtree(world: &impl GenericWorld, root: Entity, gizmos: &mut Gizmos) -> Result<()> {
//     let parent_pos = world.try_get::<Position>(root)?;

//     world
//         .children::<Connection>(root)
//         .try_for_each(|child| -> Result<()> {
//             let mut query = world.try_query_one::<(&Position, &ConnectionKind)>(child)?;
//             let (pos, kind) = query
//                 .get()
//                 .expect("Failed to execute query in draw_connections");

//             let color = match kind {
//                 ConnectionKind::Rigid => Color::green(),
//                 ConnectionKind::Spring {
//                     strength: _,
//                     dampening: _,
//                 } => Color::red(),
//             };

//             gizmos.draw(
//                 ivy_base::Line {
//                     origin: **parent_pos,
//                     dir: *(*pos - *parent_pos),
//                     radius: 0.02,
//                     corner_radius: 1.0,
//                 },
//                 color,
//             );

//             draw_subtree(world, child, gizmos)
//         })
// }

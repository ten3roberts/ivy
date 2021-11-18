use crate::Result;
use hecs::*;
use hecs_hierarchy::*;
use ivy_base::{TransformQuery, TransformQueryMut};

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

        world
            .children::<Connection>(root)
            .try_for_each(|child| -> Result<()> {
                let mut query = world.query_one::<(
                    &OffsetPosition,
                    TransformQueryMut,
                    RbQueryMut,
                    &ConnectionKind,
                    &mut Effector,
                )>(child)?;

                if let Some((offset, child_trans, child_rb, connection_kind, effector)) =
                    query.get()
                {
                    connection_kind.update(
                        offset,
                        child_trans,
                        child_rb,
                        &parent_trans,
                        &parent_rb,
                        effector,
                    );
                }
                drop(query);
                update_subtree(world, child)
            })
    } else {
        Ok(())
    }
}

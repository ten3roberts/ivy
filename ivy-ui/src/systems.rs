use hecs::{Entity, World};
use hecs_hierarchy::{Hierarchy, Parent};
use ivy_graphics::Camera;
use ivy_graphics::Result;
use ultraviolet::{Mat4, Vec3};

use crate::{constraints::ConstraintQuery, ModelMatrix, Position2D, Size2D, Widget};

/// Updates the UI tree starting from `root`, usualy the canvas.
pub fn update_ui(world: &World, root: Entity) -> Result<()> {
    let mut query = world.query_one::<(&Position2D, &Size2D)>(root)?;

    let (position, size) = query.get().ok_or(hecs::NoSuchEntity)?;

    for child in world.children::<Widget>(root) {
        apply_constaints(world, child, position, size)?;
        if world.get::<Parent<Widget>>(child).is_ok() {
            update_ui(world, child)?;
        }
    }

    Ok(())
}

/// Applies the constaints associated to entity and uses the given parent.
fn apply_constaints(
    world: &World,
    entity: Entity,
    parent_pos: &Position2D,
    parent_size: &Size2D,
) -> Result<()> {
    let mut constaints_query = world.query_one::<ConstraintQuery>(entity)?;
    let constaints = constaints_query.get().ok_or(hecs::NoSuchEntity)?;

    let mut query = world.query_one::<(&mut Position2D, &mut Size2D)>(entity)?;

    let (pos, size) = query.get().ok_or(hecs::NoSuchEntity)?;

    *pos = *parent_pos
        + Position2D(
            *constaints.abs_offset.map(|v| *v).unwrap_or_default()
                + *constaints.rel_offset.map(|v| *v).unwrap_or_default() * **parent_size,
        );

    *size = Size2D(
        *constaints.abs_size.map(|v| *v).unwrap_or_default()
            + *constaints.rel_size.map(|v| *v).unwrap_or_default() * **parent_size,
    );

    if let Some(aspect) = constaints.aspect {
        size.x = size.y * **aspect
    }

    Ok(())
}

/// Updates the canvas and all associated widgets
pub fn update_canvas(world: &World, canvas: Entity) -> Result<()> {
    let mut camera_query =
        world.query_one::<(&mut Camera, &Size2D, Option<&Position2D>)>(canvas)?;

    let (camera, size, position) = camera_query.get().ok_or(hecs::NoSuchEntity)?;

    camera.set_orthographic(size.x, size.y, 0.0, 100.0);

    if let Some(position) = position {
        camera.set_view(Mat4::from_translation(-position.xyz()))
    } else {
        camera.set_view(Mat4::from_translation(-Vec3::new(0.0, 0.0, 50.0)))
    }

    Ok(())
}

pub fn update_model_matrices(world: &World) {
    world
        .query::<(&mut ModelMatrix, &Position2D, &Size2D)>()
        .into_iter()
        .for_each(|(_, (model, pos, size))| {
            *model = ModelMatrix(
                Mat4::from_translation(pos.xyz()) * Mat4::from_nonuniform_scale(size.xyz()),
            );
        })
}

/// Satisfies all widget by adding missing ModelMatrices, Position2D and Size2D
pub fn statisfy_widgets(world: &mut World) {
    let entities = world
        .query_mut::<&Widget>()
        .into_iter()
        .map(|(e, _)| e)
        .collect::<Vec<_>>();

    entities.into_iter().for_each(|e| {
        // Ignore errors, we just collected these entities and know they exist.
        let _ = world.insert(
            e,
            (
                ModelMatrix::default(),
                Position2D::default(),
                Size2D::default(),
            ),
        );
    });
}

use crate::Result;
use hecs::{Entity, World};
use hecs_hierarchy::Hierarchy;
use ivy_core::ModelMatrix;
use ivy_graphics::Camera;
use ultraviolet::Mat4;

use crate::{constraints::ConstraintQuery, *};

/// Updates all UI trees and applies constraints.
/// Also updates canvas cameras.
pub fn update(world: &World) -> Result<()> {
    world.roots::<Widget>().iter().try_for_each(|(root, _)| {
        if world.get::<Canvas>(root).is_ok() {
            update_canvas(world, root)?;
        }

        update_from(world, root, 1)
    })
}

pub fn update_from(world: &World, parent: Entity, depth: u32) -> Result<()> {
    let mut query = world.query_one::<(&Position2D, &Size2D, Option<&mut WidgetDepth>)>(parent)?;
    let mut results = query.get().ok_or(hecs::NoSuchEntity)?;
    let position = *results.0;
    let size = *results.1;

    match results.2.as_mut() {
        Some(val) => **val = depth.into(),
        None => {}
    };

    drop(results);
    drop(query);

    world.children::<Widget>(parent).try_for_each(|child| {
        apply_constaints(world, child, position, size)?;
        assert!(parent != child);
        update_from(world, child, depth + 1)
    })
}

/// Applies the constaints associated to entity and uses the given parent.
fn apply_constaints(
    world: &World,
    entity: Entity,
    parent_pos: Position2D,
    parent_size: Size2D,
) -> Result<()> {
    let mut constaints_query = world.query_one::<ConstraintQuery>(entity)?;
    let constraints = constaints_query.get().ok_or(hecs::NoSuchEntity)?;

    let mut query = world.query_one::<(&mut Position2D, &mut Size2D)>(entity)?;

    let (pos, size) = query.get().ok_or(hecs::NoSuchEntity)?;

    *pos = parent_pos
        + Position2D(
            *constraints.abs_offset.map(|v| *v).unwrap_or_default()
                + *constraints.rel_offset.map(|v| *v).unwrap_or_default() * *parent_size,
        );

    *size = Size2D(
        *constraints.abs_size.map(|v| *v).unwrap_or_default()
            + *constraints.rel_size.map(|v| *v).unwrap_or_default() * parent_size.y,
    );

    if let Some(aspect) = constraints.aspect {
        size.x = size.y * **aspect
    }

    Ok(())
}

/// Updates the canvas view and projection
pub fn update_canvas(world: &World, canvas: Entity) -> Result<()> {
    let mut camera_query =
        world.query_one::<(&mut Camera, &Size2D, Option<&Position2D>)>(canvas)?;

    let (camera, size, position) = camera_query.get().ok_or(hecs::NoSuchEntity)?;
    let position = *position.unwrap_or(&Position2D::default());

    camera.set_orthographic(size.x * 2.0, size.y * 2.0, 0.0, 100.0);
    camera.set_view(Mat4::from_translation(-position.xyz()));

    Ok(())
}

/// Updates model matrices for UI widgets
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
        // Ignore errors, we just collected these entities and thus know they exist.
        let _ = world.insert(
            e,
            (
                ModelMatrix::default(),
                Position2D::default(),
                Size2D::default(),
                WidgetDepth::default(),
            ),
        );
    });
}

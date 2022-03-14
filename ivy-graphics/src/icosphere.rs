use std::collections::HashMap;

use glam::{vec3, Vec3};
use itertools::Itertools;
use ivy_vulkan::context::SharedVulkanContext;

use crate::{Mesh, SimpleVertex};

pub fn create_icosphere(radius: f32, iterations: usize) -> (Vec<Vec3>, Vec<u32>) {
    let x = 0.525731112119133606;
    let z = 0.850650808352039932;
    let n = 0.;
    let mut vertices = [
        vec3(-x, n, z),
        vec3(x, n, z),
        vec3(-x, n, -z),
        vec3(x, n, -z),
        // ---
        vec3(n, z, x),
        vec3(n, z, -x),
        vec3(n, -z, x),
        vec3(n, -z, -x),
        // ---
        vec3(z, x, n),
        vec3(-z, x, n),
        vec3(z, -x, n),
        vec3(-z, -x, n),
    ]
    .iter()
    .map(|v| *v * radius)
    .collect_vec();

    let mut indices = vec![
        0, 4, 1, 0, 9, 4, 9, 5, 4, 4, 5, 8, 4, 8, 1, 8, 10, 1, 8, 3, 10, 5, 3, 8, 5, 2, 3, 2, 7, 3,
        7, 10, 3, 7, 6, 10, 7, 11, 6, 11, 0, 6, 0, 1, 6, 6, 1, 10, 9, 0, 11, 9, 11, 2, 9, 2, 5, 7,
        2, 11,
    ];

    let mut new_verts = Vec::new();
    let mut new_indices = Vec::new();
    let mut edge_map: HashMap<(u32, u32), usize> = HashMap::new();
    (0..iterations).for_each(|_| {
        new_indices.clear();
        new_verts.clear();
        edge_map.clear();
        // Subdivide
        new_indices.extend(indices.chunks_exact(3).flat_map(|tri| {
            let (a, b, c) = tri
                .iter()
                .circular_tuple_windows()
                .map(|(a, b)| {
                    let edge = order_edge((*a, *b));

                    let mid = edge_map.entry(edge).or_insert_with(|| {
                        let p = ((vertices[*a as usize] + vertices[*b as usize]) / 2.0).normalize()
                            * radius;

                        let idx = vertices.len() + new_verts.len();
                        new_verts.push(p);
                        idx
                    });
                    // (vertices[edge.0 as usize] + vertices[edge.1 as usize]) / 2.0,

                    *mid as u32
                })
                .collect_tuple()
                .unwrap();

            [
                tri[0], a, c, // ---
                tri[1], b, a, // ---
                tri[2], c, b, // ---
                a, b, c,
            ]
        }));

        std::mem::swap(&mut new_indices, &mut indices);

        vertices.extend_from_slice(&new_verts);
    });
    (vertices, indices)
}

pub fn create_ico_mesh(
    context: &SharedVulkanContext,
    radius: f32,
    iterations: usize,
) -> crate::Result<Mesh<SimpleVertex>> {
    let (vertices, indices) = create_icosphere(radius, iterations);

    let vertices = vertices.iter().map(|v| SimpleVertex::new(*v));
    Mesh::new(
        context.clone(),
        &vertices.collect_vec(),
        &indices,
        Vec::new(),
    )
}

fn order_edge(edge: (u32, u32)) -> (u32, u32) {
    if edge.0 < edge.1 {
        edge
    } else {
        (edge.1, edge.0)
    }
}

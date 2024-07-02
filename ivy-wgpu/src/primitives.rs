use std::{convert::Infallible, f32::consts::PI};

use glam::{vec2, vec3, Vec4};
use ivy_assets::AssetDesc;

use crate::{mesh::Vertex, mesh_desc::MeshData};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UvSphereDesc {
    latitudes: u32,
    longitudes: u32,
}

impl Default for UvSphereDesc {
    fn default() -> Self {
        Self {
            latitudes: 32,
            longitudes: 24,
        }
    }
}

impl AssetDesc<MeshData> for UvSphereDesc {
    type Error = Infallible;

    fn load(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<MeshData>, Self::Error> {
        Ok(assets.insert(generate_uv_sphere(1.0, self.latitudes, self.longitudes)))
    }
}

fn generate_uv_sphere(radius: f32, latitudes: u32, longitudes: u32) -> MeshData {
    let latitudes = latitudes.max(3);
    let longitudes = longitudes.max(3);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let d_lat = PI / latitudes as f32;
    let d_long = 2.0 * PI / longitudes as f32;

    for i in 0..=latitudes {
        let phi = PI / 2.0 - i as f32 * d_lat;
        let xz = radius * phi.cos();
        let y = radius * phi.sin();

        for j in 0..=longitudes {
            let theta = j as f32 * d_long;

            let pos = vec3(xz * theta.cos(), y, xz * theta.sin());
            let vertex = Vertex {
                pos,
                tex_coord: vec2(i as f32 / latitudes as f32, j as f32 / longitudes as f32),
                normal: pos.normalize(),
                tangent: Vec4::ZERO,
            };

            vertices.push(vertex);
        }
    }

    for i in 0..latitudes {
        let k1 = i * (longitudes + 1);
        let k2 = k1 + longitudes + 1;

        for j in 0..longitudes {
            let k1 = k1 + j;
            let k2 = k2 + j;
            if i != 0 {
                indices.extend([k1, k2, k1 + 1]);
            }

            if i != latitudes - 1 {
                indices.extend([k1 + 1, k2, k2 + 1])
            }
        }
    }

    let mut mesh = MeshData::new(vertices.into_boxed_slice(), indices.into_boxed_slice());
    mesh.generate_tangents().unwrap();
    mesh
}

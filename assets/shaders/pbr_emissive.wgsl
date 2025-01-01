struct Object {
    world_matrix: mat4x4<f32>,
    color: vec3<f32>,
    joint_offset: u32,
}

@group(2) @binding(0)
var<storage> objects: array<Object>;

@group(2) @binding(1)
var<storage> indirection: array<u32>;

#import vertex::{VertexInput, VertexOutput, transform_vertex, Globals, globals};

#ifdef SKINNED
    @group(2) @binding(1)
    var<storage> joint_matrices: array<mat4x4<f32>>;
#endif

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let object_index = indirection[in.instance];
    let object = objects[object_index];

    var vertex = in;

    #ifdef SKINNED
    var pos = vec3(0f);

    for (var i = 0u; i < 4; i++) {
        let joint: u32 = in.joints[i];
        let weight: f32 = in.weights[i];

        pos += (joint_matrices[joint] * vec4(in.pos, 1.0)).xyz * weight;
    }

    vertex.pos = pos;
    #endif

    return transform_vertex(vertex, object.world_matrix, object.color);
}

@group(3) @binding(0)
var material_sampler: sampler;

@group(3) @binding(1)
var albedo_texture: texture_2d<f32>;

@group(3) @binding(2)
var normal_texture: texture_2d<f32>;

@group(3) @binding(3)
var mr_texture: texture_2d<f32>;

@group(3) @binding(4)
var emissive_texture: texture_2d<f32>;

@group(3) @binding(5)
var<uniform> material_data: MaterialData;

struct MaterialData {
    roughness_factor: f32,
    metallic_factor: f32,
    emissive_factor: f32,
}

#import material_pbr::{fragment_color, fragment_color_unlit, SurfaceProperties};
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let emissive = textureSample(emissive_texture, material_sampler, in.tex_coord).rgb * material_data.emissive_factor * in.color;

    let albedo = textureSample(albedo_texture, material_sampler, in.tex_coord);
    #ifdef LIT 
    let tangent_normal = textureSample(normal_texture, material_sampler, in.tex_coord).rgb * 2f - 1f;

    let metallic_roughness = textureSample(mr_texture, material_sampler, in.tex_coord);
    let metallic = material_data.metallic_factor * metallic_roughness.b;
    let roughness = material_data.roughness_factor * metallic_roughness.g;
    var surface: SurfaceProperties;

    surface.albedo = albedo;
    surface.ao = 1f;
    surface.displacement = 0f;
    surface.tangent_normal = tangent_normal;
    surface.metallic = metallic;
    surface.roughness = roughness;
    surface.emissive = emissive;

    return fragment_color(surface, in);
    #else
    return fragment_color_unlit(albedo, in);
    #endif
}

struct SkinnedVertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec4<f32>,
    @location(4) joints: vec4<u32>,
    @location(5) weights: vec4<f32>,

    @builtin(instance_index) instance: u32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) world_pos: vec3<f32>,
    @location(3) view_pos: vec3<f32>,

    @location(4) tangent_pos: vec3<f32>,

    @location(5) normal: vec3<f32>,
    @location(6) tangent: vec3<f32>,
    @location(7) bitangent: vec3<f32>,
}

struct Object {
    world_matrix: mat4x4<f32>,
}

struct Globals {
    viewproj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
}

struct Light {
    kind: u32,
    direction: vec3<f32>,
    position: vec3<f32>,
    color: vec3<f32>,
}

struct MaterialData {
    roughness_factor: f32,
    metallic_factor: f32,
}

const LIGHT_COUNT: u32 = 4;

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(2) @binding(0)
var<storage> objects: array<Object>;

@group(2) @binding(1)
var<storage> joint_matrices: array<mat4x4<f32>>;

// material
@group(3) @binding(0)
var default_sampler: sampler;

@group(3) @binding(1)
var albedo_texture: texture_2d<f32>;

@group(3) @binding(2)
var normal_texture: texture_2d<f32>;

@group(3) @binding(3)
var mr_texture: texture_2d<f32>;

@group(3) @binding(4)
var<uniform> material_data: MaterialData;

@vertex
fn vs_main(in: SkinnedVertexInput) -> VertexOutput {
    var out: VertexOutput;
    let object = objects[in.instance];

    var pos = vec3(0f);

    for (var i = 0u; i < 4; i++) {
        let joint: u32 = in.joints[i];
        let weight: f32 = in.weights[i];

        pos += (joint_matrices[joint] * vec4(in.pos, 1.0)).xyz * weight;
    }

    let world_position = object.world_matrix * vec4(pos, 1.0);

    let normal = normalize((object.world_matrix * vec4(in.normal, 0)).xyz);
    let tangent = normalize((object.world_matrix * vec4(in.tangent.xyz, 0)).xyz);
    let bitangent = normalize(cross(in.tangent.xyz, in.normal)) * in.tangent.w;

    let tbn = transpose(mat3x3(tangent, bitangent, normal));

    out.pos = globals.viewproj * world_position;
    out.tex_coord = in.tex_coord;
    out.world_pos = world_position.xyz;
    out.view_pos = (globals.view * world_position).xyz;

    out.normal = normal;
    out.tangent = tangent;
    out.bitangent = bitangent;
    out.tangent_pos = tbn * world_position.xyz;

    return out;
}

#import pbr_base::{PbrLuminance, brdf_forward};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = textureSample(albedo_texture, default_sampler, in.tex_coord).rgb;

    let tangent_normal = textureSample(normal_texture, default_sampler, in.tex_coord).rgb * 2f - 1f;

    let tbn = transpose(mat3x3(normalize(in.tangent), normalize(in.bitangent), normalize(in.normal)));

    let world_normal = normalize(transpose(tbn) * tangent_normal);

    let tangent_camera_pos = tbn * globals.camera_pos;
    let tangent_camera_dir = normalize(tangent_camera_pos - in.tangent_pos);

    let camera_dir = normalize(globals.camera_pos - in.world_pos.xyz);

    let metallic_roughness = textureSample(mr_texture, default_sampler, in.tex_coord);
    let metallic = material_data.metallic_factor * metallic_roughness.b;
    let roughness = material_data.roughness_factor * metallic_roughness.g;

    var in_lum: PbrLuminance;

    in_lum.camera_dir = camera_dir;
    in_lum.tangent_camera_dir = tangent_camera_dir;
    in_lum.world_pos = in.world_pos;
    in_lum.tangent_pos = in.tangent_pos;
    in_lum.world_normal = world_normal;
    in_lum.tangent_normal = tangent_normal;

    in_lum.albedo = albedo;
    in_lum.metallic = metallic;
    in_lum.roughness = roughness;

    in_lum.tbn = tbn;
    in_lum.view_pos = in.view_pos;

    let luminance = brdf_forward(in_lum);

    return vec4(luminance, 1);
}
struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec4<f32>,
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
    @location(8) fog: vec4<f32>,
}

struct Object {
    world_matrix: mat4x4<f32>,
}

struct Globals {
    viewproj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    fog_color: vec3<f32>,
    fog_density: f32,
}

struct MaterialData {
    roughness_factor: f32,
    metallic_factor: f32,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(2) @binding(0)
var<storage> objects: array<Object>;

// material
@group(3) @binding(0)
var material_sampler: sampler;

@group(3) @binding(1)
var albedo_texture: texture_2d<f32>;

@group(3) @binding(2)
var normal_texture: texture_2d<f32>;

@group(3) @binding(3)
var mr_texture: texture_2d<f32>;

@group(3) @binding(4)
var ao_texture: texture_2d<f32>;

@group(3) @binding(5)
var displacement_texture: texture_2d<f32>;

@group(3) @binding(6)
var<uniform> material_data: MaterialData;

const E: f32 = 2.718281828459;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let object = objects[in.instance];
    let world_position = object.world_matrix * vec4(in.pos, 1.0);

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

    let distance = length(world_position.xyz - globals.camera_pos);

    let fog_opacity = 1f - exp(-globals.fog_density * distance);
    out.fog = vec4(globals.fog_color, fog_opacity);

    return out;
}

#import pbr_base::{PbrLuminance, brdf_forward};
const DISPLACEMENT_STRENGTH: f32 = 0.2f;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = textureSample(albedo_texture, material_sampler, in.tex_coord);

    let base_color = albedo;
    let color = mix(base_color.rgb, in.fog.rgb, in.fog.a);
    return vec4(color, albedo.a);
}

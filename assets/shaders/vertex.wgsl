#define_import_path vertex

struct VertexInput {
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
    @location(8) fog: vec4<f32>,
    @location(9) color: vec3<f32>,
}

struct Globals {
    viewproj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    fog_color: vec3<f32>,
    fog_density: f32,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

fn transform_vertex(in: VertexInput, world_transform: mat4x4<f32>, color: vec3<f32>) -> VertexOutput {
    var out: VertexOutput;
    let world_position = world_transform * vec4(in.pos, 1.0);

    let normal = normalize((world_transform * vec4(in.normal, 0)).xyz);
    let tangent = normalize((world_transform * vec4(in.tangent.xyz, 0)).xyz);
    let bitangent = normalize(cross(tangent, normal)) * in.tangent.w;

    let tbn = transpose(mat3x3(tangent, bitangent, normal));

    out.pos = globals.viewproj * world_position;
    out.tex_coord = in.tex_coord;
    out.world_pos = world_position.xyz;
    out.view_pos = (globals.view * world_position).xyz;

    out.normal = normal;
    out.tangent = tangent;
    out.bitangent = bitangent;
    out.tangent_pos = tbn * world_position.xyz;
    out.color = color;

    let distance = length(world_position.xyz - globals.camera_pos);

    let fog_opacity = 1f - exp(-globals.fog_density * distance);
    out.fog = vec4(globals.fog_color, fog_opacity);

    return out;
}

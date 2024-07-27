struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec4<f32>,
    @builtin(instance_index) instance: u32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
}

struct Object {
    world_matrix: mat4x4<f32>,
}

struct Globals {
    viewproj: mat4x4<f32>,
}

const LIGHT_COUNT: u32 = 4;

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(1) @binding(0)
var<storage> objects: array<Object>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let object = objects[in.instance];
    let world_position = object.world_matrix * vec4(in.pos, 1.0);

    out.pos = globals.viewproj * world_position;
    out.normal = (object.world_matrix * vec4(in.normal, 0.0)).xyz;

    out.pos.z = clamp(out.pos.z, 0f, out.pos.w);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(vec3(in.pos.z / in.pos.w), 1f);
}

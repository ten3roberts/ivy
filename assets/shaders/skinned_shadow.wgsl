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
    @location(0) normal: vec3<f32>,
}

struct Object {
    world_matrix: mat4x4<f32>,
    joint_offset: u32,
}

struct Globals {
    viewproj: mat4x4<f32>,
}

const LIGHT_COUNT: u32 = 4;

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(1) @binding(0)
var<storage> objects: array<Object>;

@group(1) @binding(1)
var<storage> joint_matrices: array<mat4x4<f32>>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let object = objects[in.instance];

    var pos = vec3(0f);

    for (var i = 0u; i < 4; i++) {
        let joint: u32 = in.joints[i];
        let weight: f32 = in.weights[i];

        pos += (joint_matrices[object.joint_offset + joint] * vec4(in.pos, 1.0)).xyz * weight;
    }

    let world_position = object.world_matrix * vec4(pos, 1.0);

    out.pos = globals.viewproj * world_position;
    out.normal = (object.world_matrix * vec4(in.normal, 0.0)).xyz;

    out.pos.z = clamp(out.pos.z, 0f, out.pos.w);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) {}

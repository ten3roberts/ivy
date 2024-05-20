struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @builtin(instance_index) instance: u32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
}

struct Object {
    world_matrix: mat4x4<f32>,
}

struct Globals {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(1) @binding(0)
var<storage> objects: array<Object>;

// material
@group(2) @binding(0)
var default_sampler: sampler;

@group(2) @binding(1)
var diffuse_texture: texture_2d<f32>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let object = objects[in.instance];
    // out.pos = globals.proj * globals.view * object.world_matrix * vec4<f32>(in.pos, 1.0);
    out.pos = globals.proj * globals.view * object.world_matrix * vec4<f32>(in.pos, 1.0);
    out.tex_coord = in.tex_coord;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(diffuse_texture, default_sampler, in.tex_coord);
}

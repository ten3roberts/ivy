struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @builtin(instance_index) instance: u32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) world_pos: vec4<f32>,
}

struct Object {
    world_matrix: mat4x4<f32>,
}

struct Globals {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
}

struct Light {
    position: vec4<f32>,
    color: vec4<f32>,
}

const LIGHT_COUNT: u32 = 4;

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(0) @binding(1)
var<uniform> lights: array<Light, LIGHT_COUNT>;

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
    let world_position = object.world_matrix * vec4(in.pos, 1.0);
    // out.pos = globals.proj * globals.view * object.world_matrix * vec4<f32>(in.pos, 1.0);
    out.pos = globals.proj * globals.view * world_position;
    out.tex_coord = in.tex_coord;
    out.world_pos = world_position;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {


    var luminance = vec3(0f);
    for (var i = 0u; i < LIGHT_COUNT; i++) {
        let light = lights[i];
        let to_light = light.position.xyz - in.world_pos.xyz;
        let dist_sqr: f32 = dot(to_light, to_light);

        luminance += light.color.rgb / dist_sqr;
    }

    let albedo = textureSample(diffuse_texture, default_sampler, in.tex_coord);
    return albedo * vec4(luminance, 1);
}

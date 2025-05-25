struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
    @builtin(instance_index) instance: u32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) clip_pos: vec3<f32>,
    @location(1) frag_pos: vec3<f32>,
    @location(2) frag_scale: vec3<f32>,
    @location(3) color: vec4<f32>,
    @location(4) corner_radius: f32,
}

struct Object {
    world: mat4x4<f32>,
    color: vec4<f32>,
    corner_radius: f32,
}

struct Globals {
    viewproj: mat4x4<f32>,
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
}

struct MaterialData {
    roughness_factor: f32,
    metallic_factor: f32,
}


@group(0) @binding(0)
var<uniform> globals: Globals;

@group(0) @binding(1)
var<storage> objects: array<Object>;

@group(0) @binding(2)
var depth_texture: texture_2d<f32>;

@group(0) @binding(3)
var depth_sampler: sampler;


@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let object = objects[in.instance];

    let scale = vec3(
        length(object.world[0]),
        length(object.world[1]),
        length(object.world[2])
    );
    out.pos = globals.viewproj * object.world * vec4(in.pos, 1f);

    out.frag_pos = in.pos;
    out.frag_scale = scale;
    out.color = object.color * in.color;
    out.corner_radius = object.corner_radius;

    out.clip_pos = out.pos.xyz / out.pos.w;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.corner_radius <= 0.0 {
        return in.color;
    }

    let uv = vec2(in.clip_pos.x + 1.0, -in.clip_pos.y + 1.0) * 0.5;

    let depth_at = textureSample(depth_texture, depth_sampler, uv).r;

    var mask = vec4(1f);
    // if depth_at < in.clip_pos.z {
    //     mask = vec4(0.2);
    // } else {
    // }

    let width = in.frag_scale.x;
    let height = in.frag_scale.y;

    let radius = in.corner_radius * width;

    let midsegment = vec2(1f - radius, height - radius);
    let cap = vec3(
        in.frag_pos * in.frag_scale
    ) - vec3((1f - in.corner_radius) * width * sign(in.frag_pos.x), midsegment.y * sign(in.frag_pos.y), 0f);

    if abs(in.frag_pos * in.frag_scale).y > midsegment.y && cap.x * sign(in.frag_pos.x) > 0f && length(cap) > radius {
        discard;
    }

    return vec4(in.color) * mask;
}

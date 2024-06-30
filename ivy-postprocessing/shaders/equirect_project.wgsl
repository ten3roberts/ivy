struct VertexInput {
    @location(0) pos: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) vertex_pos: vec3<f32>,
}


@group(0) @binding(0)
var<uniform> projview: mat4x4<f32>;

@group(0) @binding(1)
var texture_sampler: sampler;
@group(0) @binding(2)
var texture: texture_2d<f32>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.pos = projview * vec4(in.pos, 1f);
    out.vertex_pos = in.pos;
    return out;
}

const inv_atan: vec2<f32> = vec2(0.1591f, 0.3183f);

fn sample_spherical(v: vec3<f32>) -> vec2<f32> {
    return vec2(atan2(v.z, v.x), asin(v.y)) * inv_atan + 0.5;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var uv = sample_spherical(normalize(in.vertex_pos.xyz));
    uv.y = 1f - uv.y;
    uv.x = 1f - uv.x;

    let color = textureSample(texture, texture_sampler, uv).rgb;

    return vec4(color, 1f);
}

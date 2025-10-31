struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) clip_position: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var result: VertexOutput;
    let x = i32(vertex_index) / 2;
    let y = i32(vertex_index) & 1;
    let uv = vec2<f32>(
        f32(x) * 2.0,
        f32(y) * 2.0
    );
    result.position = vec4<f32>(
        uv.x * 2.0 - 1.0,
        1.0 - uv.y * 2.0,
        1.0, 1.0
    );
    result.clip_position = result.position;
    result.uv = uv;
    return result;
}

@group(0) @binding(0)
var sharp: texture_2d<f32>;

@group(0) @binding(1)
var blurred: texture_2d<f32>;

@group(0) @binding(2)
var depth: texture_2d<f32>;

@group(0) @binding(3)
var default_sampler: sampler;

struct DofConfig {
    focus_distance: f32,
    focus_range: f32,
    near: f32,
    far: f32,
};

@group(0) @binding(4)
var<uniform> config: DofConfig;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = textureSample(depth, default_sampler, in.uv).r;
    let s = textureSample(sharp, default_sampler, in.uv);
    let b = textureSample(blurred, default_sampler, in.uv);
    let linear_d = config.near * config.far / (config.far - d * (config.far - config.near));
    let weight = clamp(abs(linear_d - config.focus_distance) / config.focus_range, 0.0, 1.0);
    return vec4(mix(s.rgb, b.rgb, weight * 0.2), s.a);
}

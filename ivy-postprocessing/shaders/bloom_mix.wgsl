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
var hdr: texture_2d<f32>;

@group(0) @binding(1)
var bloom: texture_2d<f32>;

@group(0) @binding(2)
var default_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var a = textureSample(hdr, default_sampler, in.uv).rgb;
    var b = textureSample(bloom, default_sampler, in.uv).rgb;
    return vec4(mix(a, b, 0.04), 1f);
}
 

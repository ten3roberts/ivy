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
var source_texture: texture_2d<f32>;

@group(0) @binding(1)
var default_sampler: sampler;

@group(0) @binding(2)
var<uniform> source_texel_size: vec3<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let x = source_texel_size.x;
    let y = source_texel_size.y;
    let uv = in.uv;

    // Take 13 samples around current texel:
    // a - b - c
    // - j - k -
    // d - e - f
    // - l - m -
    // g - h - i
    // === ('e' is the current texel) ===
    let a = textureSample(source_texture, default_sampler, vec2(uv.x - 2 * x, uv.y + 2 * y)).rgb;
    let b = textureSample(source_texture, default_sampler, vec2(uv.x, uv.y + 2 * y)).rgb;
    let c = textureSample(source_texture, default_sampler, vec2(uv.x + 2 * x, uv.y + 2 * y)).rgb;

    let d = textureSample(source_texture, default_sampler, vec2(uv.x - 2 * x, uv.y)).rgb;
    let e = textureSample(source_texture, default_sampler, vec2(uv.x, uv.y)).rgb;
    let f = textureSample(source_texture, default_sampler, vec2(uv.x + 2 * x, uv.y)).rgb;

    let g = textureSample(source_texture, default_sampler, vec2(uv.x - 2 * x, uv.y - 2 * y)).rgb;
    let h = textureSample(source_texture, default_sampler, vec2(uv.x, uv.y - 2 * y)).rgb;
    let i = textureSample(source_texture, default_sampler, vec2(uv.x + 2 * x, uv.y - 2 * y)).rgb;

    let j = textureSample(source_texture, default_sampler, vec2(uv.x - x, uv.y + y)).rgb;
    let k = textureSample(source_texture, default_sampler, vec2(uv.x + x, uv.y + y)).rgb;
    let l = textureSample(source_texture, default_sampler, vec2(uv.x - x, uv.y - y)).rgb;
    let m = textureSample(source_texture, default_sampler, vec2(uv.x + x, uv.y - y)).rgb;

    // Apply weighted distribution:
    // 0.5 + 0.125 + 0.125 + 0.125 + 0.125 = 1
    // a,b,d,e * 0.125
    // b,c,e,f * 0.125
    // d,e,g,h * 0.125
    // e,f,h,i * 0.125
    // j,k,l,m * 0.5
    // This shows 5 square areas that are being sampled. But some of them overlap,
    // so to have an energy preserving downsample we need to make some adjustments.
    // The weights are the distributed, so that the sum of j,k,l,m (e.g.)
    // contribute 0.5 to the final color output. The code below is written
    // to effectively yield this sum. We get:
    // 0.125*5 + 0.03125*4 + 0.0625*4 = 1
    let downsample = e * 0.125 + (a + c + g + i) * 0.03125 + (b + d + f + h) * 0.0625 + (j + k + l + m) * 0.125;

    // downsample += (a + c + g + i) * 0.03125;
    // downsample += (b + d + f + h) * 0.0625;
    // downsample += (j + k + l + m) * 0.125;
    return vec4(max(downsample, vec3(0.000001f)), 1f);
    // return vec4(1f, 0f, 0f, 1f);
}
 

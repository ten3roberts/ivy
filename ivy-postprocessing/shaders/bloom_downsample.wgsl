
struct VertexOutput {
    @builtin(position) frag_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) id: u32) -> VertexOutput {
    let uv = vec2<f32>(vec2<u32>(
        id & 1u,
        (id >> 1u) & 1u,
    ));
    var out: VertexOutput;
    // out.clip_position = vec4(uv * vec2(4.0, -4.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    out.frag_position = vec4(uv * 4.0 - 1.0, 1.0, 1.0);
    return out;
}

@group(0) @binding(0)
var source_texture: texture_2d<f32>;

@group(0) @binding(1)
var<uniform> source_resolution: f32;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let srcResolution = 1024f;
    let srcTexelSize = 1.0 / source_resolution;

    float x = srcTexelSize.x;
    float y = srcTexelSize.y;

    // Take 13 samples around current texel:
    // a - b - c
    // - j - k -
    // d - e - f
    // - l - m -
    // g - h - i
    // === ('e' is the current texel) ===
    let a = texture(source_texture, vec2(in.uv.x - 2 * x, in.uv.y + 2 * y)).rgb;
    let b = texture(source_texture, vec2(in.uv.x, in.uv.y + 2 * y)).rgb;
    let c = texture(source_texture, vec2(in.uv.x + 2 * x, in.uv.y + 2 * y)).rgb;

    let d = texture(source_texture, vec2(in.uv.x - 2 * x, in.uv.y)).rgb;
    let e = texture(source_texture, vec2(in.uv.x, in.uv.y)).rgb;
    let f = texture(source_texture, vec2(in.uv.x + 2 * x, in.uv.y)).rgb;

    let g = texture(source_texture, vec2(in.uv.x - 2 * x, in.uv.y - 2 * y)).rgb;
    let h = texture(source_texture, vec2(in.uv.x, in.uv.y - 2 * y)).rgb;
    let i = texture(source_texture, vec2(in.uv.x + 2 * x, in.uv.y - 2 * y)).rgb;

    let j = texture(source_texture, vec2(in.uv.x - x, in.uv.y + y)).rgb;
    let k = texture(source_texture, vec2(in.uv.x + x, in.uv.y + y)).rgb;
    let l = texture(source_texture, vec2(in.uv.x - x, in.uv.y - y)).rgb;
    let m = texture(source_texture, vec2(in.uv.x + x, in.uv.y - y)).rgb;

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
    return vec4(downsample, 1f);
}
 

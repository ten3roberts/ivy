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

// x: texel_size, y: textel_size, z: filter_radius
@group(0) @binding(2)
var<uniform> filter_radius: vec3<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  // The filter kernel is applied with a radius, specified in texture
    // coordinates, so that the radius will vary across mip resolutions.
    let x = filter_radius.z;
    let y = filter_radius.z;

    // Take 9 samples around current texel:
    // a - b - c
    // d - e - f
    // g - h - i
    // === ('e' is the current texel) ===
    let a = textureSample(source_texture, default_sampler, vec2(in.uv.x - x, in.uv.y + y)).rgb;
    let b = textureSample(source_texture, default_sampler, vec2(in.uv.x, in.uv.y + y)).rgb;
    let c = textureSample(source_texture, default_sampler, vec2(in.uv.x + x, in.uv.y + y)).rgb;

    let d = textureSample(source_texture, default_sampler, vec2(in.uv.x - x, in.uv.y)).rgb;
    let e = textureSample(source_texture, default_sampler, vec2(in.uv.x, in.uv.y)).rgb;
    let f = textureSample(source_texture, default_sampler, vec2(in.uv.x + x, in.uv.y)).rgb;

    let g = textureSample(source_texture, default_sampler, vec2(in.uv.x - x, in.uv.y - y)).rgb;
    let h = textureSample(source_texture, default_sampler, vec2(in.uv.x, in.uv.y - y)).rgb;
    let i = textureSample(source_texture, default_sampler, vec2(in.uv.x + x, in.uv.y - y)).rgb;

    // Apply weighted distribution, by using a 3x3 tent filter:
    //  1   | 1 2 1 |
    // -- * | 2 4 2 |
    // 16   | 1 2 1 |
    var upsample = e * 4.0;
    upsample += (b + d + f + h) * 2.0;
    upsample += (a + c + g + i);
    upsample *= 1.0 / 16.0;

    return vec4(upsample, 1f);
}
 

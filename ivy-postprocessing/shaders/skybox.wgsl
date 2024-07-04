
struct VertexOutput {
    @builtin(position) frag_position: vec4<f32>,
    @location(0) clip_position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) id: u32) -> VertexOutput {
    let uv = vec2<f32>(vec2<u32>(
        id & 1u,
        (id >> 1u) & 1u,
    ));
    var out: VertexOutput;
    // out.clip_position = vec4(uv * vec2(4.0, -4.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    out.clip_position = vec4(uv * 4.0 - 1.0, 1.0, 1.0);
    out.frag_position = vec4(uv * 4.0 - 1.0, 1.0, 1.0);
    return out;
}

struct UniformData {
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
}

@group(0) @binding(2)
var environment_map: texture_cube<f32>;

@group(0) @binding(3)
var irradiance_map: texture_cube<f32>;

@group(0) @binding(4)
var specular_map: texture_cube<f32>;

@group(1) @binding(0)
var<uniform> data: UniformData;

@group(1) @binding(1)
var skybox_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_pos_homogeneous = data.inv_proj * in.clip_position;
    let view_ray_direction = view_pos_homogeneous.xyz / view_pos_homogeneous.w;
    var ray_direction = normalize((data.inv_view * vec4(view_ray_direction, 0.0)).xyz);

    let color = textureSampleLevel(specular_map, skybox_sampler, ray_direction, 1.5).rgb;
    return vec4(color, 1.0);
    // return vec4(dir, 1.0);
}

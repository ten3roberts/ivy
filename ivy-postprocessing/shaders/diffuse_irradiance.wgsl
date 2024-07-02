
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

@group(0) @binding(0)
var<uniform> data: UniformData;

@group(0) @binding(1)
var skybox_sampler: sampler;

@group(0) @binding(2)
var skybox_texture: texture_cube<f32>;

const TAU: f32 = 6.2831853f;
const PI: f32 = 3.14159265359;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_pos_homogeneous = data.inv_proj * in.clip_position;
    let view_ray_direction = view_pos_homogeneous.xyz / view_pos_homogeneous.w;
    var ray_direction = normalize((data.inv_view * vec4(view_ray_direction, 0.0)).xyz);

    let right = normalize(cross(vec3(0f, 1f, 0f), ray_direction));

    let up = normalize(cross(ray_direction, right));

    let samples_i = 256;
    let samples_j = 128;

    var irradiance = vec3(0f);

    for (var i = 0; i < samples_i; i++) {
        let phi = TAU * ((f32(i) / f32(samples_i)));
        for (var j = 0; j < samples_j; j++) {
            let theta = 0.5 * PI * ((f32(j) / f32(samples_j)));

            // Tangent space sample dir
            let tangent_dir = vec3(
                sin(theta) * cos(phi),
                sin(theta) * sin(phi),
                cos(theta),
            );

            // World space
            let sample_dir = tangent_dir.x * right + tangent_dir.y * up + tangent_dir.z * ray_direction;

            let color = textureSample(skybox_texture, skybox_sampler, sample_dir).rgb;
            irradiance += color * cos(theta) * sin(theta);
        }
    }

    // let color = textureSample(skybox_texture, skybox_sampler, ray_direction).rgb;
    return vec4(irradiance / f32(samples_i * samples_j), 1.0);
    // return vec4(dir, 1.0);
}

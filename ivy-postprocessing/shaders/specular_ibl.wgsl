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

struct UniformData {
    inv_proj: mat4x4<f32>,
    inv_view: mat4x4<f32>,
    roughness: f32,
    resolution: u32,
}

@group(0) @binding(0)
var<uniform> data: UniformData;

@group(0) @binding(1)
var skybox_sampler: sampler;

@group(0) @binding(2)
var skybox_texture: texture_cube<f32>;

const TAU: f32 = 6.2831853f;
const PI: f32 = 3.14159265359;

fn radical_inverse_vdc(b: u32) -> f32 {
    var bits = b;
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return f32(bits) * 2.3283064365386963e-10; // / 0x100000000
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2(f32(i) / f32(n), radical_inverse_vdc(i));
}

fn importance_sample_ggx(Xi: vec2<f32>, n: vec3<f32>, roughness: f32) -> vec3<f32> {
    let a = roughness * roughness;
    let phi = TAU * Xi.x;
    let cos_theta = sqrt((1.0 - Xi.y) / (1.0 + (a * a - 1.0) * Xi.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);

    let h = vec3(cos(phi) * sin_theta, sin(phi) * sin_theta, cos_theta);

     // from tangent-space vector to world-space sample vector
    // let up = abs(N.z) < 0.999 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);
    var up = vec3(0f, 0f, 1f);
    if abs(n.z) > 0.999 {
        up = vec3(1f, 0f, 0f);
    }
    // let up = max(sign(0.999 - abs(n.z)), 0f) * vec3(0f, 0f, 1f) * (1 - max(sign(0.999 - abs(n.z)), 0f));
    let tangent = normalize(cross(up, n));
    let bitangent = cross(n, tangent);

    let sample_vec = tangent * h.x + bitangent * h.y + n * h.z;
    return normalize(sample_vec);
}

fn distribution_ggx(n: vec3<f32>, h: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let ndoth = max(dot(n, h), 0f);
    let ndoth2 = ndoth * ndoth;

    let num = a2;
    var denom = (ndoth2 * (a2 - 1f) + 1f);

    return num / denom;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_pos_homogeneous = data.inv_proj * in.clip_position;
    let view_ray_direction = view_pos_homogeneous.xyz / view_pos_homogeneous.w;
    var normal = normalize((data.inv_view * vec4(view_ray_direction, 0.0)).xyz);
    let r = normal;
    let v = r;

    let sample_count = 4096u;

    var total_weight = 0.0;
    var total_incoming = vec3(0f);

    for (var i = 0u; i < sample_count; i++) {
        let Xi = hammersley(i, sample_count);
        let h = importance_sample_ggx(Xi, normal, data.roughness);

        let l = normalize(2f * dot(v, h) * h - v);

        let ndoth = dot(normal, h);
        let hdotv = dot(v, h);
        let d = distribution_ggx(normal, h, data.roughness);

        let pdf = (d * ndoth / (4f * hdotv)) + 0.0001;

        let sa_texel = 4f * PI / (6f * f32(data.resolution) * f32(data.resolution));
        let sa_sample = 1f / (f32(sample_count) * pdf + 0.0001);

        var mip_level = 0f;
        if data.roughness > 0f {
            mip_level = 0.5 * log2(sa_sample / sa_texel);
        }

        let ndotl = dot(normal, l);
        if ndotl > 0f {
            total_incoming += textureSampleLevel(skybox_texture, skybox_sampler, l, mip_level).rgb * ndotl;
            total_weight += ndotl;
        }
    }

    return vec4(total_incoming / max(f32(total_weight), 0.0001), 1.0);
}

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
}

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


fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let a = roughness;
    let k = (a * a) / 2f;

    let num = ndotv;
    let denom = ndotv * (1.0 - k) + k;

    return num / denom;
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let ndotv = max(dot(n, v), 0f);
    let ndotl = max(dot(n, l), 0f);

    let ggx2 = geometry_schlick_ggx(ndotv, roughness);
    let ggx1 = geometry_schlick_ggx(ndotl, roughness);

    return ggx1 * ggx2;
}

fn integrate_brdf(ndotv: f32, roughness: f32) -> vec2<f32> {
    let v = vec3(sqrt(1f - ndotv * ndotv), 0f, ndotv);

    var a = 0f;
    var b = 0f;

    let n = vec3(0f, 0f, 1f);

    let sample_count = 1024u;

    for (var i = 0u; i < sample_count; i++) {
        let xi = hammersley(i, sample_count);
        let h = importance_sample_ggx(xi, n, roughness);
        let l = normalize(2.0 * dot(v, h) * h - v);

        let ndotl = max(l.z, 0f);
        let ndoth = max(h.z, 0f);
        let vdoth = max(dot(v, h), 0f);

        if ndotl > 0f {
            let g = geometry_smith(n, v, l, roughness);
            let g_vis = (g * vdoth) / (ndoth * ndotv);

            let f_c = pow(1.0 - vdoth, 5f);

            a += (1f - f_c) * g_vis;
            b += f_c * g_vis;
        }
    }

    a /= f32(sample_count);
    b /= f32(sample_count);

    return vec2(a, b);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let integrated = integrate_brdf(in.uv.x, in.uv.y);
    // let color = textureSample(skybox_texture, skybox_sampler, ray_direction).rgb;,return vec4(irradiance / f32(samples_i * samples_j), 1.0);
    return vec4(integrated, 0f, 1.0);
}

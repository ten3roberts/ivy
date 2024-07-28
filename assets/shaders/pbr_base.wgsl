#define_import_path pbr_base

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0f, 1f), 5f);
}

fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    return f0 + (max(vec3(1f - roughness), f0) - f0) * pow(clamp(1.0 - cos_theta, 0f, 1f), 5f);
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

fn geometry_schlick_ggx(ndotv: f32, roughness: f32) -> f32 {
    let r = (roughness + 1f);
    let k = (r * r) / 8f;

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

struct ShadowCamera {
    viewproj: mat4x4<f32>,
    texel_size: vec2<f32>,
    depth: f32,
}

struct Light {
    kind: u32,
    shadow_index: u32,
    shadow_cascades: u32,
    _padding: f32,
    direction: vec3<f32>,
    position: vec3<f32>,
    color: vec3<f32>,
}

@group(0) @binding(1)
var environment_map: texture_cube<f32>;

@group(0) @binding(2)
var irradiance_map: texture_cube<f32>;

@group(0) @binding(3)
var specular_map: texture_cube<f32>;

@group(0) @binding(4)
var integrated_brdf: texture_2d<f32>;

@group(0) @binding(5)
var environment_sampler: sampler;

@group(1) @binding(0)
var<storage> lights: array<Light>;

@group(1) @binding(1)
var<storage> shadow_cameras: array<ShadowCamera>;

@group(1) @binding(2)
var shadow_maps: texture_depth_2d_array;

@group(1) @binding(3)
var shadow_sampler: sampler_comparison;

struct PbrLuminance {
    camera_dir: vec3<f32>,
    tangent_camera_dir: vec3<f32>,
    world_pos: vec3<f32>,
    tangent_pos: vec3<f32>,
    world_normal: vec3<f32>,
    tangent_normal: vec3<f32>,

    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32,

    tbn: mat3x3<f32>,
    view_pos: vec3<f32>,
}

fn shadow_pcf(uv: vec2<f32>, index: u32, current_depth: f32, texel_size: vec2<f32>) -> f32 {
    var total = 0.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            total += textureSampleCompare(shadow_maps, shadow_sampler, uv + vec2(f32(x), f32(y)) * texel_size, index, current_depth);
        }
    }

    total = total / 9.0;
    return total;
}

const PI: f32 = 3.14159265359;
const U32_MAX = 0xFFFFFFFFu;

const MAX_REFLECTION_LOD: f32 = 7f;
const LIGHT_POINT: u32 = 0;
const LIGHT_DIRECTIONAL: u32 = 1;

const LIGHT_COUNT: u32 = 4;

fn pbr_luminance(in: PbrLuminance, light: Light) -> vec3<f32> {
    var l: vec3<f32>;
    var attenuation: f32;

    if light.kind == LIGHT_POINT {
        let light_position = in.tbn * light.position;
        let to_light = light_position - in.tangent_pos;
        let dist_sqr: f32 = dot(to_light, to_light);

        l = normalize(to_light);
        attenuation = 1f / dist_sqr;
    } else if light.kind == LIGHT_DIRECTIONAL {
        l = in.tbn * -light.direction;
        attenuation = 1f;
    }

    var in_light = 0f;
    var c = vec3(0f);
    if light.shadow_index != U32_MAX {
        // let in_view = globals.view * vec4(in.world_pos, 1.0);
        // let bias = max(0.05 * (1.0 - dot(vec3(0f, 0f, 1f), l)), 0.005);
        // let bias = 0.001;
        let bias = 0.0;

        var cascade_index = 0u;
        for (var i = 0u; i < light.shadow_cascades - 1; i++) {
            if in.view_pos.z < shadow_cameras[light.shadow_index + i].depth {
                cascade_index = i + 1;
            }
        }

        let shadow_camera = shadow_cameras[light.shadow_index + cascade_index];
        let light_space_clip = shadow_camera.viewproj * vec4(in.world_pos, 1.0);
        let light_space_pos = light_space_clip.xyz / light_space_clip.w;

        var light_space_uv = vec2(light_space_pos.x, -light_space_pos.y) * 0.5 + 0.5;
        let current_depth = light_space_pos.z;

        in_light = shadow_pcf(light_space_uv, light.shadow_index + cascade_index, current_depth + bias, shadow_camera.texel_size);
    }

    let h = normalize(in.tangent_camera_dir + l);

    let radiance = light.color * attenuation;

    var f0 = vec3(0.04);

    f0 = mix(f0, in.albedo, in.metallic);
    let f = fresnel_schlick(max(dot(h, in.tangent_camera_dir), 0f), f0);

    let ndf = distribution_ggx(in.tangent_normal, h, in.roughness);
    let g = geometry_smith(in.tangent_normal, in.tangent_camera_dir, l, in.roughness);

    let ndotl = max(dot(in.tangent_normal, l), 0f);

    let num = ndf * g * f;
    let denom = 4f * max(dot(in.tangent_normal, in.tangent_camera_dir), 0f) * ndotl + 0.0001;

    let specular = num / denom;

    let ks = f;
    var kd = vec3(1f) - ks;
    kd *= 1f - in.metallic;

    return in_light * (kd * in.albedo / PI + specular) * radiance * ndotl;
}

/// Calculate surface color from all incoming light
fn brdf_forward(in: PbrLuminance) -> vec3<f32> {
    var luminance = vec3(0.0) * in.albedo.rgb;

    var f0 = vec3(0.04);

    f0 = mix(f0, in.albedo, in.metallic);

    // ambient lighting
    let ambient_ks = fresnel_schlick_roughness(max(dot(in.world_normal, in.camera_dir), 0f), f0, in.roughness);
    let ambient_kd = 1f - ambient_ks;

    let r = reflect(-in.camera_dir, in.world_normal);

    let specular_color = textureSampleLevel(specular_map, environment_sampler, r, in.roughness * MAX_REFLECTION_LOD).rgb;
    let env_brdf = textureSample(integrated_brdf, environment_sampler, vec2(max(dot(in.world_normal, in.camera_dir), 0f), in.roughness)).rg;
    let specular = specular_color * (env_brdf.x + env_brdf.y);

    let irradiance = textureSample(irradiance_map, environment_sampler, in.world_normal).rgb;
    let diffuse = irradiance * in.albedo;
    let ambient_light = (ambient_kd * diffuse + ambient_ks * specular);

    luminance += ambient_light;

    for (var i = 0u; i < LIGHT_COUNT; i++) {
        let light = lights[i];
        if light.kind == U32_MAX {
            break;
        }

        luminance += pbr_luminance(in, light);
    }

    return luminance;
}

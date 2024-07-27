struct VertexInput {
    @location(0) pos: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec4<f32>,
    @builtin(instance_index) instance: u32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) world_pos: vec3<f32>,

    @location(3) tangent_pos: vec3<f32>,

    @location(4) normal: vec3<f32>,
    @location(5) tangent: vec3<f32>,
    @location(6) bitangent: vec3<f32>,
}

struct Object {
    world_matrix: mat4x4<f32>,
}

struct Globals {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
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

struct MaterialData {
    roughness_factor: f32,
    metallic_factor: f32,
}

struct ShadowCamera {
    viewproj: mat4x4<f32>,
    texel_size: vec2<f32>,
    depth: f32,
}

const LIGHT_COUNT: u32 = 4;

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(0) @binding(1)
var environment_map: texture_cube<f32>;

@group(0) @binding(2)
var irradiance_map: texture_cube<f32>;

@group(0) @binding(3)
var specular_map: texture_cube<f32>;

@group(0) @binding(4)
var integrated_brdf: texture_2d<f32>;

@group(1) @binding(0)
var<storage> lights: array<Light>;

@group(1) @binding(1)
var<storage> shadow_cameras: array<ShadowCamera>;

@group(1) @binding(2)
var shadow_maps: texture_depth_2d_array;

@group(1) @binding(3)
var shadow_sampler: sampler_comparison;

@group(2) @binding(0)
var<storage> objects: array<Object>;

// material
@group(3) @binding(0)
var default_sampler: sampler;

@group(3) @binding(1)
var albedo_texture: texture_2d<f32>;

@group(3) @binding(2)
var normal_texture: texture_2d<f32>;

@group(3) @binding(3)
var mr_texture: texture_2d<f32>;

@group(3) @binding(4)
var<uniform> material_data: MaterialData;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let object = objects[in.instance];
    let world_position = object.world_matrix * vec4(in.pos, 1.0);

    let normal = normalize((object.world_matrix * vec4(in.normal, 0)).xyz);
    let tangent = normalize((object.world_matrix * vec4(in.tangent.xyz, 0)).xyz);
    let bitangent = normalize(cross(in.tangent.xyz, in.normal)) * in.tangent.w;

    let tbn = transpose(mat3x3(tangent, bitangent, normal));

    out.pos = globals.proj * globals.view * world_position;
    out.tex_coord = in.tex_coord;
    out.world_pos = world_position.xyz;

    out.normal = normal;
    out.tangent = tangent;
    out.bitangent = bitangent;
    out.tangent_pos = tbn * world_position.xyz;

    return out;
}

const PI: f32 = 3.14159265359;
const U32_MAX = 0xFFFFFFFFu;

const LIGHT_POINT: u32 = 0;
const LIGHT_DIRECTIONAL: u32 = 1;

#import pbr_base::{fresnel_schlick, distribution_ggx, geometry_smith, fresnel_schlick_roughness};

const MAX_REFLECTION_LOD: f32 = 7f;

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

fn pbr_luminance(in: VertexOutput, tangent_camera_dir: vec3<f32>, albedo: vec3<f32>, normal: vec3<f32>, metallic: f32, roughness: f32, tbn: mat3x3<f32>, light: Light) -> vec3<f32> {
    var l: vec3<f32>;
    var attenuation: f32;

    if light.kind == LIGHT_POINT {
        let light_position = tbn * light.position;
        let to_light = light_position - in.tangent_pos.xyz;
        let dist_sqr: f32 = dot(to_light, to_light);

        l = normalize(to_light);
        attenuation = 1f / dist_sqr;
    } else if light.kind == LIGHT_DIRECTIONAL {
        l = tbn * -light.direction;
        attenuation = 1f;
    }

    var in_light = 0f;
    var c = vec3(0f);
    if light.shadow_index != U32_MAX {
        let in_view = globals.view * vec4(in.world_pos, 1.0);

        let bias = 0.001;

        var cascade_index = 0u;
        for (var i = 0u; i < light.shadow_cascades - 1; i++) {
            if in_view.z < shadow_cameras[light.shadow_index + i].depth {
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

    let h = normalize(tangent_camera_dir + l);

    let radiance = light.color * attenuation;

    var f0 = vec3(0.04);

    f0 = mix(f0, albedo, metallic);
    let f = fresnel_schlick(max(dot(h, tangent_camera_dir), 0f), f0);

    let ndf = distribution_ggx(normal, h, roughness);
    let g = geometry_smith(normal, tangent_camera_dir, l, roughness);

    let ndotl = max(dot(normal, l), 0f);

    let num = ndf * g * f;
    let denom = 4f * max(dot(normal, tangent_camera_dir), 0f) * ndotl + 0.0001;

    let specular = num / denom;

    let ks = f;
    var kd = vec3(1f) - ks;
    kd *= 1f - metallic;

    return in_light * (kd * albedo / PI + specular) * radiance * ndotl;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = textureSample(albedo_texture, default_sampler, in.tex_coord).rgb;

    let tangent_normal = textureSample(normal_texture, default_sampler, in.tex_coord).rgb * 2f - 1f;

    let tbn = transpose(mat3x3(normalize(in.tangent), normalize(in.bitangent), normalize(in.normal)));

    let world_normal = normalize(transpose(tbn) * tangent_normal);

    let tangent_camera_pos = tbn * globals.camera_pos;
    let tangent_camera_dir = normalize(tangent_camera_pos - in.tangent_pos);

    let camera_dir = normalize(globals.camera_pos - in.world_pos.xyz);

    var luminance = vec3(0.0) * albedo.rgb;

    let metallic_roughness = textureSample(mr_texture, default_sampler, in.tex_coord);
    let metallic = material_data.metallic_factor * metallic_roughness.b;
    let roughness = material_data.roughness_factor * metallic_roughness.g;

    var f0 = vec3(0.04);


    f0 = mix(f0, albedo, metallic);
    // ambient lighting
    let ambient_ks = fresnel_schlick_roughness(max(dot(world_normal, camera_dir), 0f), f0, roughness);
    let ambient_kd = 1f - ambient_ks;

    let r = reflect(-camera_dir, world_normal);

    let specular_color = textureSampleLevel(specular_map, default_sampler, r, roughness * MAX_REFLECTION_LOD).rgb;
    let env_brdf = textureSample(integrated_brdf, default_sampler, vec2(max(dot(world_normal, camera_dir), 0f), roughness)).rg;
    let specular = specular_color * (env_brdf.x + env_brdf.y);

    let irradiance = textureSample(irradiance_map, default_sampler, world_normal).rgb;
    let diffuse = irradiance * albedo;
    let ambient_light = (ambient_kd * diffuse + ambient_ks * specular);

    luminance += ambient_light;

    for (var i = 0u; i < LIGHT_COUNT; i++) {
        let light = lights[i];
        if light.kind == U32_MAX {
            break;
        }

        luminance += pbr_luminance(in, tangent_camera_dir, albedo, tangent_normal, metallic, roughness, tbn, light);
    }

    return vec4(luminance, 1);
}

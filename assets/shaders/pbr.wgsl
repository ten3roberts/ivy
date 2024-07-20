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
    _padding: vec2<f32>,
    shadow_viewproj: mat4x4<f32>,
    direction: vec3<f32>,
    position: vec3<f32>,
    color: vec3<f32>,
}

struct MaterialData {
    roughness_factor: f32,
    metallic_factor: f32,
}

const LIGHT_COUNT: u32 = 4;

@group(0) @binding(0)
var<uniform> globals: Globals;

@group(0) @binding(1)
var<uniform> lights: array<Light, LIGHT_COUNT>;

@group(0) @binding(2)
var shadow_maps: texture_depth_2d_array;

@group(0) @binding(3)
var shadow_sampler: sampler_comparison;

@group(0) @binding(4)
var environment_map: texture_cube<f32>;

@group(0) @binding(5)
var irradiance_map: texture_cube<f32>;

@group(0) @binding(6)
var specular_map: texture_cube<f32>;

@group(0) @binding(7)
var integrated_brdf: texture_2d<f32>;

@group(1) @binding(0)
var<storage> objects: array<Object>;

// material
@group(2) @binding(0)
var default_sampler: sampler;

@group(2) @binding(1)
var albedo_texture: texture_2d<f32>;

@group(2) @binding(2)
var normal_texture: texture_2d<f32>;

@group(2) @binding(3)
var mr_texture: texture_2d<f32>;

@group(2) @binding(4)
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

const PI: f32 = 3.14159265359;
const U32_MAX = 0xFFFFFFFFu;

const LIGHT_POINT: u32 = 0;
const LIGHT_DIRECTIONAL: u32 = 1;

fn pbr_luminance(world_pos: vec3<f32>, position: vec3<f32>, camera_dir: vec3<f32>, albedo: vec3<f32>, normal: vec3<f32>, metallic: f32, roughness: f32, tbn: mat3x3<f32>, light: Light) -> vec3<f32> {
    var l: vec3<f32>;
    var attenuation: f32;

    if light.kind == LIGHT_POINT {
        let light_position = tbn * light.position;
        let to_light = light_position - position.xyz;
        let dist_sqr: f32 = dot(to_light, to_light);

        l = normalize(to_light);
        attenuation = 1f / dist_sqr;
    } else if light.kind == LIGHT_DIRECTIONAL {
        l = tbn * -light.direction;
        attenuation = 1f;
    }

    var in_light = 0f;
    if light.shadow_index != U32_MAX {
        let bias = max(0.05 * (1.0 - dot(normal, l)), 0.005);

        let light_space_clip = light.shadow_viewproj * vec4(world_pos, 1.0);
        let light_space_pos = light_space_clip.xyz / light_space_clip.w;

        var light_space_uv = vec2(light_space_pos.x, -light_space_pos.y) * 0.5 + 0.5;
        let current_depth = light_space_pos.z;

        in_light = textureSampleCompare(shadow_maps, shadow_sampler, light_space_uv, light.shadow_index, current_depth - bias);
        // return vec3(current_depth - textureSample(shadow_maps, shadow_sampler, light_space_uv, light.shadow_index));
        // // return vec3(light_space_uv.x, light_space_uv.y, 0.0);
    }

    let h = normalize(camera_dir + l);

    let radiance = light.color * attenuation;

    var f0 = vec3(0.04);

    f0 = mix(f0, albedo, metallic);
    let f = fresnel_schlick(max(dot(h, camera_dir), 0f), f0);

    let ndf = distribution_ggx(normal, h, roughness);
    let g = geometry_smith(normal, camera_dir, l, roughness);

    let ndotl = max(dot(normal, l), 0f);

    let num = ndf * g * f;
    let denom = 4f * max(dot(normal, camera_dir), 0f) * ndotl + 0.0001;

    let specular = num / denom;

    let ks = f;
    var kd = vec3(1f) - ks;
    kd *= 1f - metallic;

    return in_light * (kd * albedo / PI + specular) * radiance * ndotl;
}

const MAX_REFLECTION_LOD: f32 = 7f;

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

        luminance += pbr_luminance(in.world_pos, in.tangent_pos, tangent_camera_dir, albedo, tangent_normal, metallic, roughness, tbn, light);
    }

    return vec4(luminance, 1);
}

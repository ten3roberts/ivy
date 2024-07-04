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
var environment_map: texture_cube<f32>;

@group(0) @binding(3)
var irradiance_map: texture_cube<f32>;

@group(0) @binding(4)
var specular_map: texture_cube<f32>;

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

fn pbr_luminance(position: vec3<f32>, camera_dir: vec3<f32>, albedo: vec3<f32>, normal: vec3<f32>, metallic: f32, roughness: f32, light_position: vec3<f32>, light_color: vec3<f32>) -> vec3<f32> {
    let to_light = light_position - position.xyz;
    let dist_sqr: f32 = dot(to_light, to_light);

    let l = normalize(to_light);
    let h = normalize(camera_dir + l);

    let distance = length(to_light);
    let attenuation = 1f / dist_sqr;

    let radiance = light_color * attenuation;

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

    return (kd * albedo / PI + specular) * radiance * ndotl;
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
    let ambient_ks = fresnel_schlick_roughness(max(dot(world_normal, camera_dir), 0f), f0, roughness);
    let ambient_kd = 1f - ambient_ks;
    let irradiance = textureSample(irradiance_map, default_sampler, world_normal).rgb;
    let diffuse = irradiance * albedo;
    let ambient_light = (ambient_kd * diffuse);

    luminance += ambient_light;

    for (var i = 0u; i < LIGHT_COUNT; i++) {
        let light = lights[i];

        let light_pos = tbn * light.position;

        luminance += pbr_luminance(in.tangent_pos, tangent_camera_dir, albedo, tangent_normal, metallic, roughness, light_pos, light.color);
    }

    return vec4(luminance, 1);
}

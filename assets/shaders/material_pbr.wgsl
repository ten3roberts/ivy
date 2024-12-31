#define_import_path material_pbr

#import vertex::{VertexOutput, globals};

#import pbr_base::{PbrLuminance, brdf_forward};
const DISPLACEMENT_STRENGTH: f32 = 0.2f;

struct SurfaceProperties {
    albedo: vec4<f32>,
    ao: f32,
    displacement: f32,
    tangent_normal: vec3<f32>,
    metallic: f32,
    roughness: f32,
    emissive: vec3<f32>,
}

fn fragment_color(surface: SurfaceProperties, in: VertexOutput) -> vec4<f32> {
    let tbn = transpose(mat3x3(normalize(in.tangent), normalize(in.bitangent), normalize(in.normal)));

    let world_normal = normalize(transpose(tbn) * surface.tangent_normal);

    let tangent_camera_pos = tbn * globals.camera_pos;
    let tangent_camera_dir = normalize(tangent_camera_pos - in.tangent_pos);

    let camera_dir = normalize(globals.camera_pos - in.world_pos.xyz);

    var in_lum: PbrLuminance;

    let world_pos = in.world_pos - DISPLACEMENT_STRENGTH * in.normal * (1f - surface.displacement);

    in_lum.camera_dir = camera_dir;
    in_lum.tangent_camera_dir = tangent_camera_dir;
    in_lum.world_pos = world_pos;
    in_lum.tangent_pos = tbn * world_pos;
    in_lum.world_normal = world_normal;
    in_lum.tangent_normal = surface.tangent_normal;

    in_lum.albedo = surface.albedo.rgb;
    in_lum.metallic = surface.metallic;
    in_lum.roughness = surface.roughness;
    in_lum.ao = surface.ao;

    in_lum.tbn = tbn;
    in_lum.view_pos = in.view_pos;

    let luminance = brdf_forward(in_lum);

    let color = mix(luminance, in.fog.rgb, in.fog.a) + surface.emissive;
    return vec4(color, surface.albedo.a);
}

fn fragment_color_unlit(albedo: vec4<f32>, in: VertexOutput) -> vec4<f32> {
    let base_color = albedo;
    let color = mix(base_color.rgb, in.fog.rgb, in.fog.a);
    return vec4(color, albedo.a);
}

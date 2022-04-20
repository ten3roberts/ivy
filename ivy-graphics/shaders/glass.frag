#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 fragPosition;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec4 fragColor;
layout(location = 3) in vec2 fragTexCoord;
layout(location = 4) in mat3 TBN;

layout(location = 0) out vec4 outColor;

layout(binding = 0) uniform CameraData {
    mat4 viewproj;
    mat4 view;
    mat4 projection;
    vec4 position;
} cameraData;

layout(binding = 1) uniform sampler2D screenspace;
layout(binding = 2) uniform sampler2D screenspace_d;

layout(set = 2, binding = 0) uniform sampler2D albedo;
layout(set = 2, binding = 1) uniform sampler2D normalMap;

layout(set = 2, binding = 2) uniform MaterialData {
    float roughness;
    float metallic;
    int normal;
} materialData;

vec2 fragToUv(vec3 pos) {
    vec4 ndc = (cameraData.viewproj * vec4(fragPosition, 1));
    vec2 clip = ndc.xy / ndc.w;
    return vec2((clip.x + 1) / 2, (clip.y + 1) / 2);
}

vec2 clipToUv(vec3 clip) {
    return vec2((clip.x + 1) / 2, (clip.y + 1) / 2);
}

vec2 worldToUv(vec3 w) {
    vec4 ndc = (cameraData.viewproj * vec4(w, 1));
    vec3 clip = ndc.xyz / ndc.w;
    vec2 uv = clipToUv(clip);
    return uv;
}

vec3 toClip(vec3 w) {
    vec4 ndc = (cameraData.viewproj * vec4(w, 1));
    vec3 clip = ndc.xyz / ndc.w;
    return clip;
}

vec4 raytrace(vec3 origin, vec3 dir) {
    vec4 ndc_dir = (cameraData.viewproj * vec4(dir, 0));
    vec3 origin_clip = ndc_dir.xyz / ndc_dir.w;
    vec2 origin_uv = clipToUv(origin_clip);

    float step_size = 32.0;
    float step = step_size * length(origin_clip);

    vec3 ray = origin;

    /* return vec4(vec3(texture(screenspace_d, fragTexCoord).r), 1); */
    /* return vec4(texture(screenspace, fragTexCoord).rgb, 1); */
    for (int i = 0; i < 64; i++) {
	vec4 ndc_ray = (cameraData.viewproj * vec4(ray, 1));
	vec3 clip = ndc_ray.xyz / ndc_ray.w;
	vec2 uv = clipToUv(clip);

	// Outside
	if (uv.x < 0 || uv.x > 1 || uv.y < 0 || uv.y > 1) {
	    continue;
	}

	float screen_d = texture(screenspace_d, uv).r;
	if (clip.z > screen_d  && clip.z < screen_d * 1.001 && screen_d < 0.9999) {
	    return vec4(texture(screenspace, uv).rgb, 1.);
	}

	ray += dir * step;
    }

    vec2 uv = fragToUv(fragPosition);
    return vec4(0.0);
}


// Indices of refraction
const float Air = 1.0;
const float Glass = 1.51714;

// Air to glass ratio of the indices of refraction (Eta)
const float Eta = Air / Glass;

// see http://en.wikipedia.org/wiki/Refractive_index Reflectivity
const float R0 = ((Air - Glass) * (Air - Glass)) / ((Air + Glass) * (Air + Glass));

void main() {


    vec3 normal = texture(normalMap, fragTexCoord).rgb * 2 - 1;

    normal = normalize(mix(fragNormal, TBN * normal, materialData.normal));
    vec3 incident = normalize(fragPosition - cameraData.position.xyz);
    vec3 v_refraction = refract(incident, normal, Eta);
    vec3 v_reflection = reflect(incident, normal);

    // see http://en.wikipedia.org/wiki/Schlick%27s_approximation
    float v_fresnel = R0 + (1.0 - R0) * pow((1.0 - dot(-incident, normal)), 5.0);

    vec4 albedo = texture(albedo, fragTexCoord) * fragColor;

    vec4 reflection = raytrace(fragPosition, v_reflection);

    /* vec2 uv = (cameraData.viewproj * vec4(fragPosition, 1)).xy * vec2(0.5, -0.5) + 0.5; */
    /* vec4 ndc = (cameraData.viewproj * vec4(fragPosition, 1)); */
    /* vec2 clip = ndc.xy / ndc.w; */
    /* vec2 uv = vec2((clip.x + 1) / 2, (clip.y + 1) / 2); */
    vec2 uv = fragToUv(fragPosition);

    /* vec4 refraction = raytrace(fragPosition, v_refraction); */
    float d = texture(screenspace_d, uv).r;
    /* uv = toClip(fragPosition + v_refraction * d); */
    d = toClip(vec3(normalize(fragPosition) * d)).z;

    vec2 r_uv = worldToUv(fragPosition + v_refraction * d * 0.5);
    float depth = max(sign(texture(screenspace_d, r_uv).r - d + 0.0001), 0.0);
    vec4 refraction = texture(screenspace, uv * (1.0 - depth) + r_uv * depth);
    /* vec3 refraction = raytrace(fragPosition, v_refraction).rgb; */
    /* refraction = mix(refraction, refraction * albedo.rgb, albedo.w); */

    outColor = vec4(albedo.rgb * mix(refraction.rgb, reflection.rgb, v_fresnel * reflection.a), 1.0);

}

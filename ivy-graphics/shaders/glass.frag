#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 fragPosition;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec4 fragColor;
layout(location = 3) in vec2 fragTexCoord;
layout(location = 5) in vec3 fragReflection;
layout(location = 6) in vec3 fragRefraction;
layout(location = 7) in float fragFresnel;

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

vec2 ndcToUv(vec4 ndc) {
    vec2 clip = ndc.xy / ndc.w;
    return vec2((clip.x + 1) / 2, (clip.y + 1) / 2);
}

vec3 raytrace(vec3 origin, vec3 dir) {
    vec3 ndc_dir = (cameraData.viewproj * vec4(dir, 0)).xyz;

    float step_size = 15.0;
    float step = step_size * length(ndc_dir);

    vec3 ray = origin;

    vec3 reflection = vec3(0,0,1);

    for (int i = 0; i < 10; i++) {
	vec4 ndc_ray = (cameraData.viewproj * vec4(fragPosition, 1));
	vec2 uv = ndcToUv(ndc_ray);

	float screen_d = texture(screenspace_d, uv).r;
	if (ndc_ray.z > screen_d) {
	    return texture(screenspace, uv).rgb;
	}

	ray += dir * step;
    }

    return vec3(0,0,0);
}

void main() {
    vec4 albedo = texture(albedo, fragTexCoord) * fragColor;

    vec3 reflection = raytrace(fragPosition, fragReflection);

    /* vec2 uv = (cameraData.viewproj * vec4(fragPosition, 1)).xy * vec2(0.5, -0.5) + 0.5; */
    /* vec4 ndc = (cameraData.viewproj * vec4(fragPosition, 1)); */
    /* vec2 clip = ndc.xy / ndc.w; */
    /* vec2 uv = vec2((clip.x + 1) / 2, (clip.y + 1) / 2); */
    vec2 uv = fragToUv(fragPosition);

    /* vec3 refraction = albedo.rbg * texture(screenspace, uv).rgb; */
    vec3 refraction = raytrace(fragPosition, fragRefraction);
    refraction = mix(refraction, refraction * albedo.rgb, albedo.w);
    reflection = vec3(1, 1, 1);

    outColor = vec4(mix(refraction, reflection, fragFresnel), 1.0);
}

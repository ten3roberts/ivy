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

vec2 ndcToUv(vec4 ndc) {
    vec2 clip = ndc.xy / ndc.w;
    return vec2((clip.x + 1) / 2, (clip.y + 1) / 2);
}

vec4 raytrace(vec3 origin, vec3 dir) {
    vec3 ndc_dir = (cameraData.viewproj * vec4(dir, 0)).xyz;

    float step_size = 40.0;
    float step = step_size * length(ndc_dir);

    vec3 ray = origin;

    for (int i = 0; i < 5; i++) {
	vec4 ndc_ray = (cameraData.viewproj * vec4(fragPosition, 1));
	vec2 uv = ndcToUv(ndc_ray);

	// Outside
	if (uv.x < 0 || uv.x > 1 || uv.y < 0 || uv.y > 1) {
	    continue;
	}

	float screen_d = texture(screenspace_d, uv).r;
	if (ndc_ray.z > screen_d && ndc_ray.z < screen_d * 1.01 && screen_d <
	0.99) {
	    return texture(screenspace, uv);
	}

	ray += dir * step;
    }

    vec2 uv = fragToUv(fragPosition);
    return texture(screenspace, uv);
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

    vec3 refraction = albedo.rbg * texture(screenspace, uv).rgb;
    /* vec3 refraction = raytrace(fragPosition, v_refraction).rgb; */
    refraction = mix(refraction, refraction * albedo.rgb, albedo.w);

    outColor = vec4(mix(refraction, reflection.rgb, 0.0), 1.0);
}

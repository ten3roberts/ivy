#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (input_attachment_index = 0, binding = 1) uniform subpassInput albedoBuffer;
layout (input_attachment_index = 1, binding = 2) uniform subpassInput posBuffer;
layout (input_attachment_index = 2, binding = 3) uniform subpassInput normalBuffer;
layout (input_attachment_index = 3, binding = 4) uniform subpassInput roughnessMetallicBuffer;

layout (input_attachment_index = 4, binding = 5) uniform subpassInput depthInput;

struct LightData {
	vec3 pos;
	float size;
	vec3 radiance;
	float radius;
};

layout(location = 0) in vec4 fragPosition;
layout(location = 1) in LightData light;

layout(location = 0) out vec4 outColor;

layout(binding = 0) uniform CameraData {
	mat4 viewproj;
	mat4 view;
	mat4 projection;
	vec4 pos;
} cameraData;

const float DIELECTRIC_F0 = 0.04;
const float PI = 3.1415926535897932384626433832795;

vec3 FresnelSchlick(float cosTheta, vec3 F0)
{
	return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

float DistributionGGX(vec3 normal, vec3 halfway, float roughness) {
	float a      = roughness*roughness;
	float a2     = a*a;
	float nDotH  = max(dot(normal, halfway), 0.0);
	float nDotH2 = nDotH*nDotH;

	float num   = a2;
	float denom = (nDotH2 * (a2 - 1.0) + 1.0);
	denom = PI * denom * denom;

	return num / denom;
}

float GeometrySchlickGGX(float nDotV, float roughness) {
	float r = (roughness + 1.0);
	float k = (r*r) / 8.0;

	float num   = nDotV;
	float denom = nDotV * (1.0 - k) + k;

	return num / denom;
}

float GeometrySmith(vec3 normal, vec3 cameraDir, vec3 lightDir, float roughness) {
	float nDotV = max(dot(normal, cameraDir), 0.0);
	float nDotL = max(dot(normal, lightDir), 0.0);
	float ggx2  = GeometrySchlickGGX(nDotV, roughness);
	float ggx1  = GeometrySchlickGGX(nDotL, roughness);

	return ggx1 * ggx2;
}

vec3 ClipToWorldRay(vec2 clip) {
	vec4 ray_clip = vec4(clip, -1, 1);
	vec4 ray_eye = vec4((inverse(cameraData.projection) * ray_clip).xy, -1, 0);
	vec3 ray_world = normalize((inverse(cameraData.view) * ray_eye).xyz);
	return ray_world;
}

vec3 DirectLightRadiances(float depth, vec3 worldRay) {
	vec3 Lo = vec3(0.0);

	// From camera to light vector
	vec3 toLight = (light.pos - cameraData.pos.xyz);

	vec4 lightClipSpace = cameraData.viewproj * vec4(light.pos, 1);
	float lightDepth = lightClipSpace.z / lightClipSpace.w;

	if (lightDepth > depth)
		return vec3(0,0,0);

	vec3 projected = worldRay * dot(worldRay, toLight);
	vec3 radial = projected - toLight;

	if (length(radial) < light.radius)
		Lo += light.radiance;

	return Lo;
}

vec3 PBR(vec3 albedo, vec3 pos, vec3 normal, float roughness, float metallic) {
	// Normalized to camera vector
	vec3 cameraDir = normalize(cameraData.pos.xyz - pos);

	vec3 F0 = mix(vec3(DIELECTRIC_F0), albedo, metallic);

	vec3 lightDir = normalize(light.pos - pos);
	vec3 halfway = normalize(lightDir + cameraDir);

	float dist = distance(light.pos, pos);


	vec3 radiance = light.radiance * dot(lightDir, normal) / (dist * dist);

	vec3 fresnel = FresnelSchlick(max(dot(halfway, cameraDir), 0.0), F0);

	float NDF = DistributionGGX(normal, halfway, roughness);
	float G   = GeometrySmith(normal, cameraDir, lightDir, roughness);

	vec3 numerator    = NDF * G * fresnel;
	float denominator = 4.0 * max(dot(normal, cameraDir), 0.0) * max(dot(normal, lightDir), 0.0)  + 0.00001;
	vec3 specular     = numerator / denominator;

	vec3 kS = fresnel;
	vec3 kD = vec3(1.0) - kS;

	kD *= 1.0 - metallic;

	float ndotL = max(dot(normal, lightDir), 0.0);


	return (kD * albedo / PI + specular) * radiance * ndotL;

}

/* vec3 applyFog(vec3 color, float  distance) { */

/* 	float exponent = distance * env.fog_density; */
/* 	float visibility = exp(-pow(exponent, env.fog_gradient)); */

/* 	return mix(env.fog_color, color, visibility); */
/* } */

void main() {
	float depth = subpassLoad(depthInput).x;

	vec3 ray = normalize(fragPosition.xyz - cameraData.pos.xyz);
	if (depth == 1) {
		outColor = vec4(DirectLightRadiances(1, ray), 1);
		return;
	}
	vec3 albedo = subpassLoad(albedoBuffer).xyz;

	vec2 roughnessMetallic = subpassLoad(roughnessMetallicBuffer).xy;
	float roughness = roughnessMetallic.x;
	float metallic = roughnessMetallic.y;

	vec3 normal = subpassLoad(normalBuffer).xyz;
	vec3 pos = subpassLoad(posBuffer).xyz;

	vec3 color = vec3(0);

	if (depth != 1) {
		color += PBR(albedo, pos, normal, roughness, metallic);
	}

	color += DirectLightRadiances(depth, ray);

	float distance = length(cameraData.pos.xyz - pos);

	/* color = applyFog(color, distance); */

	outColor = vec4(color, 1);
}

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

layout(set = 1, binding = 0) uniform sampler2D screenspace;
layout(set = 1, binding = 1) uniform sampler2D screenspace_d;

layout(set = 3, binding = 0) uniform sampler2D albedo;
layout(set = 3, binding = 1) uniform sampler2D normalMap;

layout(set = 3, binding = 2) uniform MaterialData {
	float roughness;
	float metallic;
	int normal;
} materialData;

void main() {
	vec4 albedo = texture(albedo, fragTexCoord) * fragColor;

	/* vec4 refractionColor = texture(u_cubemap, normalize(v_refraction)); */

	// Trace


	vec4 reflectionColor = vec4(1.0, 1.0, 1.0, 1.0);
	/* vec4 reflectionColor = texture(u_cubemap, normalize(v_reflection)); */

	outColor = vec4(reflectionColor.rgb, fragFresnel);
	outColor = albedo;
}

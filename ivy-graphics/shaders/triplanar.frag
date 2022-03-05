#version 450
#extension GL_ARB_separate_shader_objects : enable


layout(location = 0) in vec3 fragPosition;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec4 fragColor;
layout(location = 3) in vec2 fragTexCoord;
layout(location = 4) in mat3 TBN;

layout(location = 0) out vec4 outAlbedo;
layout(location = 1) out vec4 outPosition;
layout(location = 2) out vec3 outNormal;
layout(location = 3) out vec2 metallicRoughness;
layout(location = 4) out vec4 transparent;

layout(set = 2, binding = 0) uniform sampler2D albedo;
layout(set = 2, binding = 1) uniform sampler2D normalMap;

layout(set = 2, binding = 2) uniform MaterialData {
	float roughness;
	float metallic;
	int normal;
} materialData;

struct TriUv {
	vec2 x;
	vec2 y;
	vec2 z;
};

TriUv create_tri_uv(vec3 pos) {
	return TriUv(
		pos.zy,
		pos.zx,
		pos.xy
	);
}

void main() {

	TriUv tri = create_tri_uv(fragPosition);

	vec3 bias = abs(fragNormal);


	vec2 texCoord;

	if (bias.x > bias.y && bias.x > bias.z) {
		texCoord = tri.x;
	} else if (bias.y > bias.z) {
		texCoord = tri.y;
	} else {
		texCoord = tri.z;
	}

	texCoord *= 0.5;

	outAlbedo = texture(albedo, texCoord) * fragColor;
	transparent = vec4(outAlbedo.xyz, 1 - outAlbedo.w);
	outPosition = vec4(fragPosition, 0);
	vec3 normal = texture(normalMap, texCoord).rgb * 2 - 1;

	outNormal = normalize(mix(fragNormal, TBN * normal, materialData.normal));

	metallicRoughness = vec2(materialData.roughness, materialData.metallic);
}

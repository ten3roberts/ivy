#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 fragPosition;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec2 fragTexCoord;

layout(location = 0) out vec4 outAlbedo;
layout(location = 1) out vec4 outPosition;
layout(location = 2) out vec3 outNormal;
layout(location = 3) out vec2 metallicRoughness;

layout(set = 2, binding = 0) uniform sampler2D albedo; 

layout(set = 2, binding = 1) uniform MaterialData {
  float roughness;
  float metallic;
} materialData;

void main() {
  outAlbedo = vec4(texture(albedo, fragTexCoord).rgb, 1);
  outPosition = vec4(fragPosition, 0);
  outNormal = normalize(fragNormal);

  metallicRoughness = vec2(materialData.roughness, materialData.metallic);
}

#version 450
#extension GL_ARB_separate_shader_objects : enable


layout(location = 0) in vec3 fragPosition;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec4 fragColor;
layout(location = 3) in vec2 fragTexCoord;

layout(location = 0) out vec4 outDiffuse;

layout(set = 2, binding = 0) uniform sampler2D albedo;

layout(set = 2, binding = 1) uniform MaterialData {
  float roughness;
  float metallic;
} materialData;

void main() {
  outDiffuse = vec4(texture(albedo, fragTexCoord).rgb, 1) * fragColor;
}

#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 fragPosition;
layout(location = 1) in vec3 fragNormal;
layout(location = 2) in vec2 fragTexCoord;

layout(location = 0) out vec4 outAlbedo;
layout(location = 1) out vec4 outPosition;
layout(location = 2) out vec3 outNormal;

layout(set = 2, binding = 0) uniform sampler2D albedo; 

void main() {
  outAlbedo = texture(albedo, fragTexCoord);
  outPosition = vec4(fragPosition, 0);
  outNormal = fragNormal;
}

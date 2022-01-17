#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 texCoord;
layout(location = 3) in vec3 tangent;

layout(location = 0) out vec3 fragPosition;
layout(location = 1) out vec3 fragNormal;
layout(location = 2) out vec4 fragColor;
layout(location = 3) out vec2 fragTexCoord;
layout(location = 4) out mat3 TBN;

layout(binding = 0) uniform CameraData {
  mat4 viewproj;
  mat4 view;
  mat4 projection;
  vec4 position;
} cameraData;

struct ObjectData {
  mat4 model;
  vec4 color;
};

layout(std140,set = 1, binding = 0) readonly buffer ObjectBuffer{
  ObjectData objects[];
} objectBuffer;

void main() {
  ObjectData objectData = objectBuffer.objects[gl_InstanceIndex];

  fragTexCoord = texCoord;
  fragColor = objectData.color;

  mat4 model = objectData.model;

  vec4 pos = model * vec4(inPosition, 1);

  fragPosition = pos.xyz;
  fragNormal = normalize((model * vec4(normal, 0.0)).xyz);

  vec3 bitangent = cross(normal, tangent);
  vec3 T = vec3(model * vec4(tangent,   0.0));
  vec3 B = vec3(model * vec4(bitangent, 0.0));
  vec3 N = vec3(model * vec4(normal,    0.0));
  TBN = mat3(T, B, N);

  gl_Position = cameraData.viewproj * pos;
}

#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 inPosition;

struct LightData {
  vec3 pos;
  float size;
  vec3 radiance;
  float radius;
};

layout(location = 0) out vec4 fragPosition;
layout(location = 1) out LightData light;

layout(binding = 0) uniform CameraData {
  mat4 viewproj;
  mat4 view;
  mat4 projection;
  vec4 position;
} cameraData;

layout(set = 1, binding = 0) readonly buffer LightBuffer {
  LightData objects[];
} lightBuffer;

void main() {
  light = lightBuffer.objects[gl_InstanceIndex];

  vec4 pos = vec4(inPosition * light.size + light.pos, 1);

  fragPosition = pos;

  gl_Position = cameraData.viewproj * pos;
}

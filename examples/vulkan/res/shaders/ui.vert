#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec2 texCoord;

layout(location = 0) out vec2 fragTexCoord;

layout(binding = 0) uniform CameraData {
  mat4 viewproj;
  vec4 position;
} cameraData;

struct ObjectData {
  mat4 mvp;
};

layout(std140,set = 1, binding = 0) readonly buffer ObjectBuffer{ 
  ObjectData objects[];
} objectBuffer;

void main() {
  ObjectData objectData = objectBuffer.objects[gl_BaseInstance];

  fragTexCoord = texCoord;
  gl_Position = cameraData.viewproj * objectData.mvp * vec4(inPosition, 1);
}
#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 texCoord;

layout(location = 0) out vec3 fragModelPosition;
layout(location = 1) out vec4 fragColor;

layout(binding = 0) uniform CameraData {
  mat4 viewproj;
  mat4 view;
  mat4 projection;
  vec4 position;
} cameraData;

layout ( push_constant ) uniform ObjectData {
  mat4 model; 
  vec4 color;
} objectData;

void main() {
  mat4 modelView = cameraData.view * objectData.model;
  mat4 proj = cameraData.projection;

  // First colunm.
  modelView[0][0] = objectData.model[0][0]; 
  modelView[0][1] = 0.0; 
  modelView[0][2] = 0.0; 

  // Second colunm.
  modelView[1][0] = 0.0; 
  modelView[1][1] = objectData.model[1][1]; 
  modelView[1][2] = 0.0; 

  // Third colunm.
  modelView[2][0] = 0.0; 
  modelView[2][1] = 0.0; 
  modelView[2][2] = objectData.model[2][2];

  vec4 pos = vec4(inPosition, 1);

  fragModelPosition = inPosition;
  fragColor = objectData.color;

  gl_Position = proj * modelView * pos;

}

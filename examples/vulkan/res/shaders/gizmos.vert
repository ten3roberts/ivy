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
  vec4 billboard_axis;
} objectData;

mat3 axisBillboard(vec3 up, vec3 viewDir) {
  // Looking from the up axis
  if (abs(dot(up, viewDir)) > 0.999) {
    mat3 result;
    result[0][0] = 1;
    result[1][1] = 1;
    result[2][2] = 1;
  }

  vec3 right = normalize(cross(up, viewDir));
  vec3 forward = cross(right, up);

  mat3 result;
  result[0].xyz = right;
  result[1].xyz = up;
  result[2].xyz = forward;

  return result;
}

void main() {

  mat4 view = cameraData.view;
  mat4 proj = cameraData.projection;

  if (length(objectData.billboard_axis) > 0.0) {
    vec4 pos = objectData.model * vec4(0, 0, 0, 1);

    vec3 viewDir = normalize(pos.xyz -
      cameraData.position.xyz);

    mat3 billboard = axisBillboard(objectData.billboard_axis.xyz, viewDir);
    vec3 newPos = billboard * inPosition;

    gl_Position = proj * view * objectData.model * vec4(newPos, 1);
  } else {
    mat4 modelView = cameraData.view * objectData.model;

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
    gl_Position = proj * modelView * pos;
  }

  fragModelPosition = inPosition;
  fragColor = objectData.color;


}

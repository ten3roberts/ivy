#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (input_attachment_index = 0, set = 0, binding = 0) uniform subpassInput
diffuse;

layout (input_attachment_index = 1, set = 0, binding = 1) uniform subpassInput
wireframe;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec4 fragPosition;

layout(location = 0) out vec4 outColor;

/* layout(set = 2, binding = 0) uniform sampler2D albedo; */ 

const float E = 2.7182818284;

void main() {
  float dropoff = -100;
  /* float along_diagonal = length(( fragTexCoord - vec2(0, 1)) * 0.707106781187); */
  float blend = 1/ (1 + pow(E, dropoff * (fragTexCoord.x - 0.5)));

  outColor = mix(subpassLoad(diffuse), subpassLoad(wireframe), blend);
  /* outColor = vec4(0, along_diagonal, 0, 1); */
}

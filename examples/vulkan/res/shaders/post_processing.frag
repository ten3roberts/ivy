#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(binding = 0) uniform sampler2D diffuse;
layout(binding = 1) uniform sampler2D wireframe;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec4 fragPosition;

layout(location = 0) out vec4 outColor;

/* layout(set = 2, binding = 0) uniform sampler2D albedo; */ 

const float E = 2.7182818284;

void main() {
  float dropoff = -100;
  /* float along_diagonal = length(( fragTexCoord - vec2(0, 1)) * 0.707106781187); */
  float blend = 1/ (1 + pow(E, dropoff * (fragTexCoord.x - 0.5)));

  outColor = mix(texture(diffuse, fragTexCoord), texture(wireframe, fragTexCoord), blend);
  /* outColor = vec4(0, along_diagonal, 0, 1); */
}

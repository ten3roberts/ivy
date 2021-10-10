#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 fragModelPosition;
layout(location = 1) in vec4 fragColor;

layout(location = 0) out vec4 outColor;

void main() {
  if (length(fragModelPosition) > 1)
      discard;

  outColor = fragColor;
}

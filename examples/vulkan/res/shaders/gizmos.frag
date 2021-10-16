#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 fragModelPosition;
layout(location = 1) in vec4 fragColor;

layout(location = 0) out vec4 outColor;

layout (input_attachment_index = 0, binding = 0, set = 1) uniform subpassInput depthInput;

void main() {
  float depth = subpassLoad(depthInput).x;
  float currentDepth = gl_FragCoord.z;
  float opacity = 1.0;

  if (currentDepth > depth)
  opacity = 0.2;

  if (length(fragModelPosition) > 1)
  discard;

  outColor = fragColor * vec4(1,1,1,opacity);
}

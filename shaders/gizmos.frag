#version 460
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec3 modelPosition;
layout(location = 1) in vec2 texCoord;
layout(location = 2) in vec4 color;
layout(location = 3) in vec3 scale;
layout(location = 4) in float cornerRadius;

layout(location = 0) out vec4 outColor;

layout (input_attachment_index = 0, binding = 0, set = 1) uniform subpassInput depthInput;

void main() {
  float depth = subpassLoad(depthInput).x;
  float currentDepth = gl_FragCoord.z;
  float alpha = 1.0;
  bool obscured = false;

  if (currentDepth > depth) {
    obscured = true;
  }

  float h = scale.y;
  float w = scale.x;

  float radius = cornerRadius * w;


  float midSegment = (h  - radius);
  float midSegmentX = (1-radius);

  vec3 cap = (modelPosition * scale) - vec3((1-cornerRadius) * w * sign(modelPosition.x), midSegment *
    sign(modelPosition.y), 0); 

  if (abs((modelPosition * scale).y) > midSegment && (cap.x * sign(modelPosition.x) > 0) && length(cap) > radius) {
    alpha = 0;
  }

  outColor = color * vec4(1,1,1,alpha) * (obscured ? vec4(0.2) : vec4(1));
}

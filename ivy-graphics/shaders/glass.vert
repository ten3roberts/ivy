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
layout(location = 5) out vec3 fragReflection;
layout(location = 6) out vec3 fragRefraction;
layout(location = 7) out float fragFresnel;


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


// Indices of refraction
const float Air = 1.0;
const float Glass = 1.51714;

// Air to glass ratio of the indices of refraction (Eta)
const float Eta = Air / Glass;

// see http://en.wikipedia.org/wiki/Refractive_index Reflectivity
const float R0 = ((Air - Glass) * (Air - Glass)) / ((Air + Glass) * (Air + Glass));

layout(std140, set = 1, binding = 0) readonly buffer ObjectBuffer{
  ObjectData objects[];
} objectBuffer;

void main() {
  ObjectData objectData = objectBuffer.objects[gl_InstanceIndex];

  fragTexCoord = texCoord;
  fragColor = objectData.color;

  mat4 model = objectData.model;

  vec4 pos = model * vec4(inPosition, 1);

  vec3 incident = normalize((pos - cameraData.position).xyz);

  fragNormal = normalize((model * vec4(normal, 0.0)).xyz);
  fragRefraction = refract(incident, fragNormal, Eta);
  fragReflection = reflect(incident, fragNormal);

  // see http://en.wikipedia.org/wiki/Schlick%27s_approximation
  fragFresnel = R0 + (1.0 - R0) * pow((1.0 - dot(-incident, fragNormal)), 5.0);

  fragPosition = pos.xyz;

  gl_Position = cameraData.viewproj * pos;
}

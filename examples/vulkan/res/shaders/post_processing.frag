#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (input_attachment_index = 0, set = 0, binding = 0) uniform subpassInput
albedoBuffer;
layout (input_attachment_index = 1, set = 0, binding = 1) uniform subpassInput
positionBuffer;
layout (input_attachment_index = 2, set = 0, binding = 2) uniform subpassInput
normalBuffer;
layout(location = 0) out vec4 outColor;

struct LightData {
  vec3 position;
  float intensity;
  vec3 color;
  float distance_to_center;
};

layout(set = 1, binding = 0) uniform LightSceneData {
  uint num_lights;
} lightSceneData;

layout(set = 1, binding = 1) readonly buffer LightBufferData {
  LightData lights[]; 
} lightBuffer;


void main() {
  /* outColor = subpassLoad(albedo) / pow(length(subpassLoad(position)), 2); */

  vec3 albedo = subpassLoad(albedoBuffer).xyz;

  float illuminance = 0;
  vec3 normal = normalize(subpassLoad(normalBuffer).xyz);
  vec3 pos = subpassLoad(positionBuffer).xyz;

  for (int i = 0; i < lightSceneData.num_lights; i++) {
    LightData light = lightBuffer.lights[i];
    vec3 lightDir = light.position - pos;
    float lightDistSqr = lightDir.x * lightDir.x + lightDir.y * lightDir.y +
      lightDir.z * lightDir.z;

    float brightness =
      clamp(dot(normalize(lightDir), normal)
      * light.intensity / lightDistSqr, 0, 1);

    illuminance += light.color * brightness;
  }

  outColor = vec4(diffuse, 1);
}

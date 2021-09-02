#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (input_attachment_index = 0, binding = 0) uniform subpassInput
albedoBuffer;
layout (input_attachment_index = 1, binding = 1) uniform subpassInput
posBuffer;
layout (input_attachment_index = 2, binding = 2) uniform subpassInput
normalBuffer;
layout (input_attachment_index = 3, binding = 3) uniform subpassInput
roughnessMetallicBuffer;

layout(location = 0) out vec4 outColor;

struct LightData {
  vec3 pos;
  float reference_illuminance;
  vec3 radiance;
};

layout(binding = 4) uniform CameraData {
  mat4 viewproj;
  vec4 pos;
} cameraData;

layout(binding = 5) uniform LightSceneData {
  vec3 ambient;
  uint num_lights;
} lightSceneData;

layout(binding = 6) readonly buffer LightBufferData {
  LightData lights[]; 
} lightBuffer;


const float DIELECTRIC_F0 = 0.04;
const float PI = 3.1415926535897932384626433832795;

vec3 FresnelSchlick(float cosTheta, vec3 F0)
{
  return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

float DistributionGGX(vec3 normal, vec3 halfway, float roughness)
{
  float a      = roughness*roughness;
  float a2     = a*a;
  float nDotH  = max(dot(normal, halfway), 0.0);
  float nDotH2 = nDotH*nDotH;

  float num   = a2;
  float denom = (nDotH2 * (a2 - 1.0) + 1.0);
  denom = PI * denom * denom;

  return num / denom;
}

float GeometrySchlickGGX(float nDotV, float roughness)
{
  float r = (roughness + 1.0);
  float k = (r*r) / 8.0;

  float num   = nDotV;
  float denom = nDotV * (1.0 - k) + k;

  return num / denom;
}

float GeometrySmith(vec3 normal, vec3 cameraDir, vec3 lightDir, float roughness)
{
  float nDotV = max(dot(normal, cameraDir), 0.0);
  float nDotL = max(dot(normal, lightDir), 0.0);
  float ggx2  = GeometrySchlickGGX(nDotV, roughness);
  float ggx1  = GeometrySchlickGGX(nDotL, roughness);

  return ggx1 * ggx2;
}

vec3 PBR(vec3 albedo, vec3 pos, vec3 normal, float roughness, float metallic) {
  // Normalized to camera vector
  vec3 cameraDir = normalize(cameraData.pos.xyz - pos);

    vec3 F0 = mix(vec3(DIELECTRIC_F0), albedo, metallic);

  vec3 Lo = vec3(0.0);


  for (int i = 0; i < lightSceneData.num_lights; i++) {
    LightData light = lightBuffer.lights[i];
    vec3 lightDir = normalize(light.pos - pos);
    vec3 halfway = normalize(lightDir + cameraDir);

    float dist = distance(light.pos, pos);
    vec3 radiance = light.radiance * dot(lightDir, normal) / (dist * dist);


    vec3 fresnel = FresnelSchlick(max(dot(halfway, cameraDir), 0.0), F0);

    float NDF = DistributionGGX(normal, halfway, roughness);       
    float G   = GeometrySmith(normal, cameraDir, lightDir, roughness);       


    vec3 numerator    = NDF * G * fresnel;
    float denominator = 4.0 * max(dot(normal, cameraDir), 0.0) * max(dot(normal, lightDir), 0.0)  + 0.0001;
    vec3 specular     = numerator / denominator;

    vec3 kS = fresnel;
    vec3 kD = vec3(1.0) - kS;

    kD *= 1.0 - metallic;

    float ndotL = max(dot(normal, lightDir), 0.0);        
    Lo += (kD * albedo / PI + specular) * radiance * ndotL;
  }

  return Lo + lightSceneData.ambient * albedo;
}

void main() {
  /* outColor = subpassLoad(albedo) / pow(length(subpassLoad(pos)), 2); */

  vec3 albedo = subpassLoad(albedoBuffer).xyz;

  vec2 roughnessMetallic = subpassLoad(roughnessMetallicBuffer).xy;
  float roughness = roughnessMetallic.x;
  float metallic = roughnessMetallic.y;

  vec3 normal = normalize(subpassLoad(normalBuffer).xyz);
  vec3 pos = subpassLoad(posBuffer).xyz;

  vec3 color = PBR(albedo, pos, normal, roughness, metallic);
  outColor = vec4(color, 1);
}

#version 450
#extension GL_ARB_separate_shader_objects : enable

layout (input_attachment_index = 0, binding = 1) uniform subpassInput lit;
layout (input_attachment_index = 1, binding = 2) uniform subpassInput depthInput;

layout(location = 0) in vec4 fragPosition;
layout(location = 1) in vec2 fragUv;

layout(location = 0) out vec4 outColor;

layout(binding = 0) uniform CameraData {
	mat4 viewproj;
	mat4 view;
	mat4 projection;
	vec4 pos;
	vec4 forward;
} cameraData;

layout(binding = 3) uniform EnvData {
	vec3 ambient;
	float fog_density;
	vec3 fog_color;
	float fog_gradient;
} env;

vec3 fog(vec3 color, float  distance) {

	float exponent = distance * env.fog_density;
	float visibility = exp(-pow(exponent, env.fog_gradient));

	return mix(env.fog_color, color, visibility);
}

vec3 depth_to_world(float d)
{
	vec4 screenPos;
	screenPos.x = fragUv.x*2.0f-1.0f;
	screenPos.y = -(fragUv.y*2.0f-1.0f);
	screenPos.z = d;
	screenPos.w = 1.0f;

	vec4 worldPos = inverse(cameraData.viewproj) * screenPos;
	worldPos /= worldPos.w;
	return worldPos.xyz;
}

void main() {
	vec3 world = depth_to_world(subpassLoad(depthInput).x);
	float depth = length(world - cameraData.pos.xyz);

	vec3 color = subpassLoad(lit).rgb;
	color += env.ambient * color;

	color = fog(color, depth);

	outColor = vec4(color, 1);
}

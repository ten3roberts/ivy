#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

layout(set = 2, binding = 0) uniform sampler2D albedo; 

void main() {
    vec4 sampled = texture(albedo, fragTexCoord);

		if (sampled.a < 0.9)
				discard;

		outColor = sampled;
}

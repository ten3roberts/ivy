#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec4 fragColor;
layout(location = 1) in vec2 fragTexCoord;

layout(location = 0) out vec4 outColor;

layout(set = 2, binding = 0) uniform sampler2D albedo; 

void main() {
    outColor = mix(texture(albedo, fragTexCoord), vec4(fragTexCoord, 0.0, 1.0), 0.5);
}

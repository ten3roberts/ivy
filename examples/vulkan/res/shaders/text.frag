#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec2 fragTexCoord;
layout(location = 1) in vec4 fragPos;

layout(location = 0) out vec4 outColor;

layout(set = 2, binding = 0) uniform sampler2D atlas; 

void main() {
    float sampled = texture(atlas, fragTexCoord).r;

    vec3 textColor = vec3(1);
    outColor = vec4(textColor, sampled);
}
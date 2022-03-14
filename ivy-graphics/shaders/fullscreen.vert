#version 450

layout(location = 0) out vec4 position;
layout(location = 1) out vec2 uv;

void main()
{
    uv = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
    position = vec4(uv * 2.0f - 1.0f, 0.0f, 1.0f);
    gl_Position = position;
    /* fragPosition =  gl_Position; */
}

#version 450

layout(location = 0) out vec4 position;

void main() 
{
	vec2 fragTexCoord = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
	position = vec4(fragTexCoord * 2.0f - 1.0f, 0.0f, 1.0f);
	gl_Position = position;
  /* fragPosition =  gl_Position; */
}

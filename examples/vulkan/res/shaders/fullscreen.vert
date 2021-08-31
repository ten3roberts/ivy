#version 450

void main() 
{
	vec2 fragTexCoord = vec2((gl_VertexIndex << 1) & 2, gl_VertexIndex & 2);
	gl_Position = vec4(fragTexCoord * 2.0f - 1.0f, 0.0f, 1.0f);
  /* fragPosition =  gl_Position; */
}

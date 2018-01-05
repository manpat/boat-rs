attribute vec3 position;
attribute vec3 color;

uniform mat4 proj;
uniform mat4 view;

varying vec3 v_color;

void main() {
	vec4 world_pos = vec4(position, 1.0);
	gl_Position = proj * view * world_pos;
	v_color = color;
}

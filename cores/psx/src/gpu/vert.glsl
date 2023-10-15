#version 330 core
in ivec2 vertex_position;
in uvec3 vertex_color;

out vec3 color;

void main() {
    float x = (float(vertex_position.x) / 512) - 1.0;
    float y = (float(vertex_position.y) / 256) - 1.0;

    gl_Position.xyzw = vec4(x, y, 0.0, 1.0);
    color = vec3(float(vertex_color.r) / 255,
                 float(vertex_color.g) / 255,
                 float(vertex_color.b) / 255);
}
#version 450

// layout(location = 0) in vec4 a_Pos;
layout(location = 0) in vec2 pos;
// layout(location = 0) out vec4 gl_Position;
out gl_PerVertex {
    vec4 gl_Position;
};

void main() {
    // vec2 position = vec2(gl_VertexIndex, (gl_VertexIndex & 1) * 2) - 1;
    // vec2 position = vec2(a_Pos.x, (a_Pos.x & 1))
    // gl_Position = vec4(a_Pos.xy, 0.0, 1.0);
    gl_Position = vec4(pos, 0.0, 1.0);
}

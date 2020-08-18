#version 450

layout(location = 0) out vec4 fragColor;

void main() {
    fragColor = vec4(gl_FragCoord.x / 1120.0, gl_FragCoord.y / 700.0, 1.0, 0.0);
}

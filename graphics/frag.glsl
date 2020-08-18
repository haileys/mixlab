#version 450

layout(location = 0) out vec4 fragColor;

void main() {
    // fragColor = vec4(gl_FragCoord.xy, 0.0, 1.0);
    fragColor = vec4(gl_FragCoord.x, 0.0, 1.0, 0.0);
}

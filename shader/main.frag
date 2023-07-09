#version 450
#include <global.glsl>

layout(push_constant) uniform PushConstants {
    mat4 model;
    Material material;
    Camera camera;
} pc;

layout (location = 0) in vec4 o_color;
layout (location = 1) in vec2 o_uv;
layout (location = 0) out vec4 uFragColor;

void main() { 
    // uFragColor = texture(textures[0], o_uv);
    if (pc.material.base_color_texture_index != -1)
        uFragColor = texture(u_textures[pc.material.base_color_texture_index], o_uv);
    else if (pc.material.base_color != vec3(0.0))
        uFragColor = vec4(pc.material.base_color, 1.0);
    else
        uFragColor = vec4(1.0, 0.0, 1.0, 1.0);
}
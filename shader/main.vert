#version 450
#include <global.glsl>

layout(push_constant) uniform PushConstants {
    mat4 model;
    Material material;
    Camera camera;
} pc;

layout (location = 0) in vec3 position;
layout (location = 1) in vec3 normal;
layout (location = 2) in vec2 uv;
layout (location = 3) in vec3 tangent;
layout (location = 4) in vec4 color;

layout (location = 0) out vec4 o_color;
layout (location = 1) out vec2 o_uv;

void main() {
    // Transform the vertex position from model to clip space.
    // The position should be a vec4 with the w component as 1.0 to apply translation.
    vec4 local_to_world = pc.model * vec4(position, 1.0);
    vec4 world_to_clip = pc.camera.view_proj * local_to_world;

    // The clip space position is then assigned to gl_Position, a built-in output variable.
    gl_Position = world_to_clip;
    o_color = color;
    o_uv = uv;
}
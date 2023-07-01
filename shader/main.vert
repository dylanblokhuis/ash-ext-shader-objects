#version 450
#extension GL_ARB_separate_shader_objects : enable
#extension GL_ARB_shading_language_420pack : enable
#extension GL_EXT_buffer_reference2 : enable

layout (buffer_reference) buffer Camera {
    mat4 view_proj;
    mat4 inverse_view_proj;
    mat4 view;
    mat4 inverse_view;
    mat4 proj;
    mat4 inverse_proj;
    vec3 world_position;
};

layout(push_constant) uniform Transform {
    mat4 model;
} pc;

layout(set = 0, binding = 0) uniform UniformBufferObject {
  Camera camera;
  vec2 _pad;
} ubo;

layout (location = 0) in vec3 position;
layout (location = 1) in vec3 normal;
layout (location = 2) in vec2 uv;
layout (location = 3) in vec3 tangent;
layout (location = 4) in vec4 color;

layout (location = 0) out vec4 o_color;

void main() {
    // Transform the vertex position from model to clip space.
    // The position should be a vec4 with the w component as 1.0 to apply translation.
    vec4 local_to_world = pc.model * vec4(position, 1.0);
    vec4 world_to_clip = ubo.camera.view_proj * local_to_world;

    // The clip space position is then assigned to gl_Position, a built-in output variable.
    gl_Position = world_to_clip;
    o_color = color;
}
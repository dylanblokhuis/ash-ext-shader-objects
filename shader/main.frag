#version 450
#extension GL_ARB_separate_shader_objects : enable
#extension GL_ARB_shading_language_420pack : enable
#extension GL_EXT_buffer_reference2 : enable
#extension GL_EXT_nonuniform_qualifier : enable

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
layout(set = 0, binding = 1) uniform sampler2D textures[];

layout (location = 0) in vec4 o_color;
layout (location = 1) in vec2 o_uv;
layout (location = 0) out vec4 uFragColor;

void main() { 
    uFragColor = texture(textures[0], o_uv);
}
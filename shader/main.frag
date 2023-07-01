#version 450
#extension GL_ARB_separate_shader_objects : enable
#extension GL_ARB_shading_language_420pack : enable
#extension GL_EXT_buffer_reference2 : enable

layout (buffer_reference) buffer Camera {
  mat4 proj;
  mat4 inverse_proj;
  mat4 view;
  mat4 inverse_view;
  mat4 inverse_view_proj;
  mat4 view_proj;
  vec3 world_position;
};

layout(push_constant) uniform Transform {
    mat4 model;
} pc;

layout(set = 0, binding = 0) uniform UniformBufferObject {
  Camera camera;
  vec2 _pad;
} ubo;

layout (location = 0) in vec4 o_color;
layout (location = 0) out vec4 uFragColor;

void main() {
    uFragColor = o_color;
}
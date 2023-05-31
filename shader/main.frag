#version 450
#extension GL_ARB_separate_shader_objects : enable
#extension GL_ARB_shading_language_420pack : enable
#extension GL_EXT_buffer_reference2 : enable

layout (buffer_reference) buffer MyBuffer {
  vec4 color;
};

layout(set = 0, binding = 0) uniform UniformBufferObject {
  MyBuffer my_buffer;
  vec2 _pad;
} ubo;

layout (location = 0) in vec4 o_color;
layout (location = 0) out vec4 uFragColor;

void main() {
    uFragColor = ubo.my_buffer.color;
}
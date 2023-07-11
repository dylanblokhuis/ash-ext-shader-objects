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

layout (buffer_reference) buffer Material {
    vec3 base_color;
    int base_color_texture_index;
    vec3 emissive;
    int emissive_texture_index;
    float perceptual_roughness;
    float metallic;
    int metallic_roughness_texture_index;
    float reflectance;
    int normal_map_texture_index;
    int flip_normal_map_y;
    int occlusion_texture_index;
    float depth_bias;
};

layout(set = 0, binding = 0) uniform texture2D u_textures[];
layout(set = 0, binding = 1) uniform sampler sampler_nlr;
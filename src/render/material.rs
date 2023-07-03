use ash::vk::CullModeFlags;
use bevy::prelude::*;
use bevy::reflect::TypeUuid;

use super::image::Image;

#[derive(Debug, TypeUuid, Clone)]
#[uuid = "8ecbac0f-f545-4473-ad43-e1f4243af51e"]
pub struct Material {
    pub base_color: Vec3,
    pub base_color_texture: Option<Handle<Image>>,
    pub emissive: Vec3,
    pub emissive_texture: Option<Handle<Image>>,
    pub perceptual_roughness: f32,
    pub metallic: f32,
    pub metallic_roughness_texture: Option<Handle<Image>>,
    pub reflectance: f32,
    pub normal_map_texture: Option<Handle<Image>>,
    pub flip_normal_map_y: bool,
    pub occlusion_texture: Option<Handle<Image>>,
    pub cull_mode: Option<CullModeFlags>,
    pub double_sided: bool,
    // for z-fighting
    pub depth_bias: f32,
}

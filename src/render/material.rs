use ash::vk::CullModeFlags;
use bevy::prelude::*;
use bevy::reflect::TypeUuid;

use super::image::Image;

#[derive(Debug, TypeUuid, Clone)]
#[uuid = "c94c1494-85e5-4a4c-8575-48baadfef3ab"]
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

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub base_color: Vec3,
    pub base_color_texture_index: i32,
    pub emissive: Vec3,
    pub emissive_texture_index: i32,
    pub perceptual_roughness: f32,
    pub metallic: f32,
    pub metallic_roughness_texture_index: i32,
    pub reflectance: f32,
    pub normal_map_texture_index: i32,
    pub flip_normal_map_y: i32,
    pub occlusion_texture_index: i32,
    pub depth_bias: f32,
}

impl MaterialUniform {
    pub fn from_material(material: Material) -> Self {
        Self {
            base_color: material.base_color,
            base_color_texture_index: 0,
            emissive: material.emissive,
            emissive_texture_index: 0,
            perceptual_roughness: material.perceptual_roughness,
            metallic: material.metallic,
            metallic_roughness_texture_index: 0,
            reflectance: material.reflectance,
            normal_map_texture_index: 0,
            flip_normal_map_y: material.flip_normal_map_y.into(),
            occlusion_texture_index: 0,
            depth_bias: material.depth_bias,
        }
    }
}

impl Default for Material {
    fn default() -> Self {
        Material {
            // White because it gets multiplied with texture values if someone uses
            // a texture.
            base_color: Vec3::new(1.0, 1.0, 1.0),
            base_color_texture: None,
            emissive: Vec3::new(0.0, 0.0, 0.0),
            emissive_texture: None,
            // Matches Blender's default roughness.
            perceptual_roughness: 0.5,
            // Metallic should generally be set to 0.0 or 1.0.
            metallic: 0.0,
            metallic_roughness_texture: None,
            // Minimum real-world reflectance is 2%, most materials between 2-5%
            // Expressed in a linear scale and equivalent to 4% reflectance see
            // <https://google.github.io/filament/Material%20Properties.pdf>
            reflectance: 0.5,
            occlusion_texture: None,
            normal_map_texture: None,
            flip_normal_map_y: false,
            double_sided: false,
            cull_mode: Some(CullModeFlags::BACK),
            depth_bias: 0.0,
            // unlit: false,
            // fog_enabled: true,
            // alpha_mode: AlphaMode::Opaque,
            // depth_bias: 0.0,
            // depth_map: None,
            // parallax_depth_scale: 0.1,
            // max_parallax_layer_count: 16.0,
            // parallax_mapping_method: ParallaxMappingMethod::Occlusion,
        }
    }
}

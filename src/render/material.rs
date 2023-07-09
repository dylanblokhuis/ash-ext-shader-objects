use ash::vk::CullModeFlags;
use bevy::prelude::*;
use bevy::reflect::{TypePath, TypeUuid};

use super::image::Image;

#[derive(Debug, TypeUuid, Clone, TypePath)]
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
    pub unlit: bool,
    pub alpha_mode: AlphaMode,
}

#[repr(C, align(16))]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Default)]
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
    pub fn from_material(material: &Material) -> Self {
        Self {
            base_color: material.base_color,
            base_color_texture_index: -1,
            emissive: material.emissive,
            emissive_texture_index: -1,
            perceptual_roughness: material.perceptual_roughness,
            metallic: material.metallic,
            metallic_roughness_texture_index: -1,
            reflectance: material.reflectance,
            normal_map_texture_index: -1,
            flip_normal_map_y: material.flip_normal_map_y.into(),
            occlusion_texture_index: -1,
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
            unlit: false,
            // fog_enabled: true,
            alpha_mode: AlphaMode::Opaque,
            // depth_bias: 0.0,
            // depth_map: None,
            // parallax_depth_scale: 0.1,
            // max_parallax_layer_count: 16.0,
            // parallax_mapping_method: ParallaxMappingMethod::Occlusion,
        }
    }
}

// TODO: add discussion about performance.
/// Sets how a material's base color alpha channel is used for transparency.
#[derive(Debug, Default, Reflect, Copy, Clone, PartialEq)]
#[reflect(Default, Debug)]
pub enum AlphaMode {
    /// Base color alpha values are overridden to be fully opaque (1.0).
    #[default]
    Opaque,
    /// Reduce transparency to fully opaque or fully transparent
    /// based on a threshold.
    ///
    /// Compares the base color alpha value to the specified threshold.
    /// If the value is below the threshold,
    /// considers the color to be fully transparent (alpha is set to 0.0).
    /// If it is equal to or above the threshold,
    /// considers the color to be fully opaque (alpha is set to 1.0).
    Mask(f32),
    /// The base color alpha value defines the opacity of the color.
    /// Standard alpha-blending is used to blend the fragment's color
    /// with the color behind it.
    Blend,
    /// Similar to [`AlphaMode::Blend`], however assumes RGB channel values are
    /// [premultiplied](https://en.wikipedia.org/wiki/Alpha_compositing#Straight_versus_premultiplied).
    ///
    /// For otherwise constant RGB values, behaves more like [`AlphaMode::Blend`] for
    /// alpha values closer to 1.0, and more like [`AlphaMode::Add`] for
    /// alpha values closer to 0.0.
    ///
    /// Can be used to avoid “border” or “outline” artifacts that can occur
    /// when using plain alpha-blended textures.
    Premultiplied,
    /// Combines the color of the fragments with the colors behind them in an
    /// additive process, (i.e. like light) producing lighter results.
    ///
    /// Black produces no effect. Alpha values can be used to modulate the result.
    ///
    /// Useful for effects like holograms, ghosts, lasers and other energy beams.
    Add,
    /// Combines the color of the fragments with the colors behind them in a
    /// multiplicative process, (i.e. like pigments) producing darker results.
    ///
    /// White produces no effect. Alpha values can be used to modulate the result.
    ///
    /// Useful for effects like stained glass, window tint film and some colored liquids.
    Multiply,
}

impl Eq for AlphaMode {}

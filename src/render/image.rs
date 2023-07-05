use bevy::reflect::{Reflect, TypeUuid};
use image::DynamicImage;

use crate::{buffer::TextureDescriptor, ctx::SamplerDesc};

#[derive(Reflect, Debug, Clone, TypeUuid)]
#[uuid = "6ea26da6-6cf8-4ea2-9986-1d7bf6c17d6f"]
#[reflect_value]
pub struct Image {
    pub data: DynamicImage,
    // pub texture_descriptor: TextureDescriptor,
    pub sampler_descriptor: SamplerDesc,
}

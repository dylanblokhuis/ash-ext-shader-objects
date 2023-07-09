use anyhow::Result;
use ash::vk;
use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    reflect::{Reflect, TypeUuid},
    utils::BoxedFuture,
};
use image::{DynamicImage, GenericImageView};

use crate::ctx::SamplerDesc;

#[derive(Reflect, Debug, Clone, TypeUuid)]
#[uuid = "6ea26da6-6cf8-4ea2-9986-1d7bf6c17d6f"]
#[reflect_value]
pub struct Image {
    pub data: DynamicImage,
    pub format: vk::Format,
    pub sampler_descriptor: SamplerDesc,
}

pub struct ImageTextureLoader;

const FILE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];

impl AssetLoader for ImageTextureLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<()>> {
        Box::pin(async move {
            // use the file extension for the image type
            let ext = load_context.path().extension().unwrap().to_str().unwrap();

            let img = Image {
                data: image::load_from_memory(bytes).expect("Failed to load image"),
                format: extension_to_vk_format(ext),
                sampler_descriptor: SamplerDesc {
                    texel_filter: vk::Filter::LINEAR,
                    mipmap_mode: vk::SamplerMipmapMode::LINEAR,
                    address_modes: vk::SamplerAddressMode::REPEAT,
                },
            };

            println!("{:?} {:?}", img.data.dimensions(), ext);

            load_context.set_default_asset(LoadedAsset::new(img));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        FILE_EXTENSIONS
    }
}

fn extension_to_vk_format(ext: &str) -> vk::Format {
    match ext {
        "png" => vk::Format::R8G8B8A8_SRGB,
        "jpg" => vk::Format::R8G8B8A8_UNORM,
        "jpeg" => vk::Format::R8G8B8A8_UNORM,
        _ => panic!("Unsupported image format"),
    }
}

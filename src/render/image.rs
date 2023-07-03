use bevy::reflect::{Reflect, TypeUuid};

use crate::{buffer::TextureDescriptor, ctx::SamplerDesc};

#[derive(Reflect, Debug, Clone, TypeUuid)]
#[uuid = "6ea26da6-6cf8-4ea2-9986-1d7bf6c17d6f"]
#[reflect_value]
pub struct Image {
    pub data: Vec<u8>,
    // TODO: this nesting makes accessing Image metadata verbose. Either flatten out descriptor or add accessors
    pub texture_descriptor: TextureDescriptor,
    /// The [`ImageSampler`] to use during rendering.
    pub sampler_descriptor: SamplerDesc,
    // pub texture_view_descriptor: Option<>,
}

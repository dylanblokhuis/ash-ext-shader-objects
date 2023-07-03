// use std::{collections::HashMap, path::Path};

// use bevy::{
//     asset::{AssetIoError, AssetLoader, LoadContext},
//     utils::{BoxedFuture, HashSet},
// };
// use gltf::{
//     accessor::Iter,
//     mesh::{util::ReadIndices, Mode},
//     texture::{MagFilter, MinFilter, WrappingMode},
//     Material, Node, Primitive,
// };
// use thiserror::Error;

// /// An error that occurs when loading a glTF file.
// #[derive(Error, Debug)]
// pub enum GltfError {
//     #[error("unsupported primitive mode")]
//     UnsupportedPrimitive { mode: Mode },
//     #[error("invalid glTF file: {0}")]
//     Gltf(#[from] gltf::Error),
//     #[error("binary blob is missing")]
//     MissingBlob,
//     #[error("failed to decode base64 mesh data")]
//     Base64Decode(#[from] base64::DecodeError),
//     #[error("unsupported buffer format")]
//     BufferFormatUnsupported,
//     #[error("invalid image mime type: {0}")]
//     InvalidImageMimeType(String),
//     // #[error("You may need to add the feature for the file format: {0}")]
//     // ImageError(#[from] TextureError),
//     #[error("failed to load an asset path: {0}")]
//     AssetIoError(#[from] AssetIoError),
//     #[error("Missing sampler for animation {0}")]
//     MissingAnimationSampler(usize),
//     // #[error("failed to generate tangents: {0}")]
//     // GenerateTangentsError(#[from] bevy_render::mesh::GenerateTangentsError),
//     // #[error("failed to generate morph targets: {0}")]
//     // MorphTarget(#[from] bevy_render::mesh::morph::MorphBuildError),
// }

// /// Loads glTF files with all of their data as their corresponding bevy representations.
// pub struct GltfLoader;

// impl AssetLoader for GltfLoader {
//     fn load<'a>(
//         &'a self,
//         bytes: &'a [u8],
//         load_context: &'a mut LoadContext,
//     ) -> BoxedFuture<'a, anyhow::Result<()>> {
//         Box::pin(async move { Ok(load_gltf(bytes, load_context, self).await?) })
//     }

//     fn extensions(&self) -> &[&str] {
//         &["gltf", "glb"]
//     }
// }

// /// Loads an entire glTF file.
// async fn load_gltf<'a, 'b>(
//     bytes: &'a [u8],
//     load_context: &'a mut LoadContext<'b>,
//     loader: &GltfLoader,
// ) -> Result<(), GltfError> {
//     let gltf = gltf::Gltf::from_slice(bytes)?;
//     let buffer_data = load_buffers(&gltf, load_context, load_context.path()).await?;

//     let mut materials = vec![];
//     let mut named_materials = HashMap::default();
//     let mut linear_textures = HashSet::default();
//     for material in gltf.materials() {
//         let handle = load_material(&material, load_context);
//         if let Some(name) = material.name() {
//             named_materials.insert(name.to_string(), handle.clone());
//         }
//         materials.push(handle);
//         if let Some(texture) = material.normal_texture() {
//             linear_textures.insert(texture.texture().index());
//         }
//         if let Some(texture) = material.occlusion_texture() {
//             linear_textures.insert(texture.texture().index());
//         }
//         if let Some(texture) = material
//             .pbr_metallic_roughness()
//             .metallic_roughness_texture()
//         {
//             linear_textures.insert(texture.texture().index());
//         }
//     }

//     Ok(())
// }

// struct DataUri<'a> {
//     mime_type: &'a str,
//     base64: bool,
//     data: &'a str,
// }
// fn split_once(input: &str, delimiter: char) -> Option<(&str, &str)> {
//     let mut iter = input.splitn(2, delimiter);
//     Some((iter.next()?, iter.next()?))
// }

// impl<'a> DataUri<'a> {
//     fn parse(uri: &'a str) -> Result<DataUri<'a>, ()> {
//         let uri = uri.strip_prefix("data:").ok_or(())?;
//         let (mime_type, data) = split_once(uri, ',').ok_or(())?;

//         let (mime_type, base64) = match mime_type.strip_suffix(";base64") {
//             Some(mime_type) => (mime_type, true),
//             None => (mime_type, false),
//         };

//         Ok(DataUri {
//             mime_type,
//             base64,
//             data,
//         })
//     }

//     fn decode(&self) -> Result<Vec<u8>, base64::DecodeError> {
//         if self.base64 {
//             base64::decode(self.data)
//         } else {
//             Ok(self.data.as_bytes().to_owned())
//         }
//     }
// }

// /// Loads the raw glTF buffer data for a specific glTF file.
// async fn load_buffers(
//     gltf: &gltf::Gltf,
//     load_context: &LoadContext<'_>,
//     asset_path: &Path,
// ) -> Result<Vec<Vec<u8>>, GltfError> {
//     const VALID_MIME_TYPES: &[&str] = &["application/octet-stream", "application/gltf-buffer"];

//     let mut buffer_data = Vec::new();
//     for buffer in gltf.buffers() {
//         match buffer.source() {
//             gltf::buffer::Source::Uri(uri) => {
//                 let uri = percent_encoding::percent_decode_str(uri)
//                     .decode_utf8()
//                     .unwrap();
//                 let uri = uri.as_ref();
//                 let buffer_bytes = match DataUri::parse(uri) {
//                     Ok(data_uri) if VALID_MIME_TYPES.contains(&data_uri.mime_type) => {
//                         data_uri.decode()?
//                     }
//                     Ok(_) => return Err(GltfError::BufferFormatUnsupported),
//                     Err(()) => {
//                         // TODO: Remove this and add dep
//                         let buffer_path = asset_path.parent().unwrap().join(uri);
//                         load_context.read_asset_bytes(buffer_path).await?
//                     }
//                 };
//                 buffer_data.push(buffer_bytes);
//             }
//             gltf::buffer::Source::Bin => {
//                 if let Some(blob) = gltf.blob.as_deref() {
//                     buffer_data.push(blob.into());
//                 } else {
//                     return Err(GltfError::MissingBlob);
//                 }
//             }
//         }
//     }

//     Ok(buffer_data)
// }

// /// Loads a glTF material as a bevy [`StandardMaterial`] and returns it.
// fn load_material(material: &Material, load_context: &mut LoadContext) -> Handle<StandardMaterial> {
//     let material_label = material_label(material);

//     let pbr = material.pbr_metallic_roughness();

//     let color = pbr.base_color_factor();
//     let base_color_texture = pbr.base_color_texture().map(|info| {
//         // TODO: handle info.tex_coord() (the *set* index for the right texcoords)
//         let label = texture_label(&info.texture());
//         let path = AssetPath::new_ref(load_context.path(), Some(&label));
//         load_context.get_handle(path)
//     });

//     let normal_map_texture: Option<Handle<Image>> =
//         material.normal_texture().map(|normal_texture| {
//             // TODO: handle normal_texture.scale
//             // TODO: handle normal_texture.tex_coord() (the *set* index for the right texcoords)
//             let label = texture_label(&normal_texture.texture());
//             let path = AssetPath::new_ref(load_context.path(), Some(&label));
//             load_context.get_handle(path)
//         });

//     let metallic_roughness_texture = pbr.metallic_roughness_texture().map(|info| {
//         // TODO: handle info.tex_coord() (the *set* index for the right texcoords)
//         let label = texture_label(&info.texture());
//         let path = AssetPath::new_ref(load_context.path(), Some(&label));
//         load_context.get_handle(path)
//     });

//     let occlusion_texture = material.occlusion_texture().map(|occlusion_texture| {
//         // TODO: handle occlusion_texture.tex_coord() (the *set* index for the right texcoords)
//         // TODO: handle occlusion_texture.strength() (a scalar multiplier for occlusion strength)
//         let label = texture_label(&occlusion_texture.texture());
//         let path = AssetPath::new_ref(load_context.path(), Some(&label));
//         load_context.get_handle(path)
//     });

//     let emissive = material.emissive_factor();
//     let emissive_texture = material.emissive_texture().map(|info| {
//         // TODO: handle occlusion_texture.tex_coord() (the *set* index for the right texcoords)
//         // TODO: handle occlusion_texture.strength() (a scalar multiplier for occlusion strength)
//         let label = texture_label(&info.texture());
//         let path = AssetPath::new_ref(load_context.path(), Some(&label));
//         load_context.get_handle(path)
//     });

//     load_context.set_labeled_asset(
//         &material_label,
//         LoadedAsset::new(StandardMaterial {
//             base_color: Color::rgba_linear(color[0], color[1], color[2], color[3]),
//             base_color_texture,
//             perceptual_roughness: pbr.roughness_factor(),
//             metallic: pbr.metallic_factor(),
//             metallic_roughness_texture,
//             normal_map_texture,
//             double_sided: material.double_sided(),
//             cull_mode: if material.double_sided() {
//                 None
//             } else {
//                 Some(Face::Back)
//             },
//             occlusion_texture,
//             emissive: Color::rgb_linear(emissive[0], emissive[1], emissive[2]),
//             emissive_texture,
//             unlit: material.unlit(),
//             alpha_mode: alpha_mode(material),
//             ..Default::default()
//         }),
//     )
// }

// use std::{collections::HashMap, path::Path};

// use ash::vk::{CullModeFlags, PrimitiveTopology};
// use bevy::{
//     asset::{AssetIoError, AssetLoader, AssetPath, LoadContext, LoadedAsset},
//     math::vec3,
//     prelude::*,
//     utils::{BoxedFuture, HashSet},
// };
// use gltf::{
//     accessor::Iter,
//     mesh::{util::ReadIndices, Mode},
//     texture::{MagFilter, MinFilter, WrappingMode},
//     Node, Primitive,
// };
// use thiserror::Error;

// use super::{image::Image, material::AlphaMode, mesh::Mesh};

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
//     let mut named_materials: HashMap<String, Handle<crate::Material>> = HashMap::default();
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

//     let mut meshes = vec![];
//     let mut named_meshes = HashMap::default();
//     for gltf_mesh in gltf.meshes() {
//         let mut primitives = vec![];
//         for primitive in gltf_mesh.primitives() {
//             let primitive_label = primitive_label(&gltf_mesh, &primitive);
//             let primitive_topology = get_primitive_topology(primitive.mode())?;

//             let mut mesh = Mesh {
//                 primitive_topology,
//                 indices: vec![],
//                 vertices: vec![],
//             };

//             // Read vertex attributes
//             for (semantic, accessor) in primitive.attributes() {
//                 let view = accessor.view().unwrap();
//                 let reader = accessor.reader(|buffer| Some(&buffer.view().unwrap().data()));
//                 let count = accessor.count();

//                 // Read data based on attribute semantic
//                 match semantic {
//                     gltf::Semantic::Positions => {
//                         if let Some(gltf::accessor::ReadVertices::F32(iter)) =
//                             reader.read_vertices()
//                         {
//                             for vertex in iter.take(count) {
//                                 let position: [f32; 3] = vertex.into();
//                                 mesh.vertices.push(Vertex {
//                                     position,
//                                     ..Default::default()
//                                 });
//                             }
//                         }
//                     }
//                     gltf::Semantic::Normals => {
//                         if let Some(gltf::accessor::ReadVertices::F32(iter)) =
//                             reader.read_vertices()
//                         {
//                             for (vertex, normal) in mesh.vertices.iter_mut().zip(iter.take(count)) {
//                                 vertex.normal = normal.into();
//                             }
//                         }
//                     }
//                     gltf::Semantic::TexCoords(_) => {
//                         if let Some(gltf::accessor::ReadVertices::F32(iter)) =
//                             reader.read_vertices()
//                         {
//                             for (vertex, uv) in mesh.vertices.iter_mut().zip(iter.take(count)) {
//                                 vertex.uv = uv.into();
//                             }
//                         }
//                     }
//                     gltf::Semantic::Tangents => {
//                         if let Some(gltf::accessor::ReadVertices::F32(iter)) =
//                             reader.read_vertices()
//                         {
//                             for (vertex, tangent) in mesh.vertices.iter_mut().zip(iter.take(count))
//                             {
//                                 vertex.tangent = tangent.into();
//                             }
//                         }
//                     }
//                     gltf::Semantic::Colors(_) => {
//                         if let Some(gltf::accessor::ReadVertices::F32(iter)) =
//                             reader.read_vertices()
//                         {
//                             for (vertex, color) in mesh.vertices.iter_mut().zip(iter.take(count)) {
//                                 vertex.color = color.into();
//                             }
//                         }
//                     }
//                     _ => {}
//                 }
//             }

//             // Read vertex indices
//             let reader = primitive.reader(|buffer| Some(buffer_data[buffer.index()].as_slice()));
//             if let Some(indices) = reader.read_indices() {
//                 mesh.indices = match indices {
//                     ReadIndices::U32(iter) => iter.collect(),
//                     ReadIndices::U16(iter) => iter.map(|i| i as u32).collect(),
//                     ReadIndices::U8(iter) => iter.map(|i| i as u32).collect(),
//                 };
//             };

//             {
//                 let morph_target_reader = reader.read_morph_targets();
//                 if morph_target_reader.len() != 0 {
//                     let morph_targets_label = morph_targets_label(&gltf_mesh, &primitive);
//                     let morph_target_image = MorphTargetImage::new(
//                         morph_target_reader.map(PrimitiveMorphAttributesIter),
//                         mesh.count_vertices(),
//                     )?;
//                     let handle = load_context.set_labeled_asset(
//                         &morph_targets_label,
//                         LoadedAsset::new(morph_target_image.0),
//                     );

//                     mesh.set_morph_targets(handle);
//                     let extras = gltf_mesh.extras().as_ref();
//                     if let Option::<MorphTargetNames>::Some(names) =
//                         extras.and_then(|extras| serde_json::from_str(extras.get()).ok())
//                     {
//                         mesh.set_morph_target_names(names.target_names);
//                     }
//                 }
//             }

//             if mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_none()
//                 && matches!(mesh.primitive_topology, PrimitiveTopology::TRIANGLE_LIST)
//             {
//                 let vertex_count_before = mesh.count_vertices();
//                 mesh.duplicate_vertices();
//                 mesh.compute_flat_normals();
//                 let vertex_count_after = mesh.count_vertices();

//                 if vertex_count_before != vertex_count_after {
//                     bevy_log::debug!("Missing vertex normals in indexed geometry, computing them as flat. Vertex count increased from {} to {}", vertex_count_before, vertex_count_after);
//                 } else {
//                     bevy_log::debug!(
//                         "Missing vertex normals in indexed geometry, computing them as flat."
//                     );
//                 }
//             }

//             if let Some(vertex_attribute) = reader
//                 .read_tangents()
//                 .map(|v| VertexAttributeValues::Float32x4(v.collect()))
//             {
//                 mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, vertex_attribute);
//             } else if mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some()
//                 && primitive.material().normal_texture().is_some()
//             {
//                 bevy_log::debug!(
//                     "Missing vertex tangents, computing them using the mikktspace algorithm"
//                 );
//                 if let Err(err) = mesh.generate_tangents() {
//                     bevy_log::warn!(
//                         "Failed to generate vertex tangents using the mikktspace algorithm: {:?}",
//                         err
//                     );
//                 }
//             }

//             let mesh = load_context.set_labeled_asset(&primitive_label, LoadedAsset::new(mesh));
//             primitives.push(super::GltfPrimitive {
//                 mesh,
//                 material: primitive
//                     .material()
//                     .index()
//                     .and_then(|i| materials.get(i).cloned()),
//                 extras: get_gltf_extras(primitive.extras()),
//                 material_extras: get_gltf_extras(primitive.material().extras()),
//             });
//         }

//         let handle = load_context.set_labeled_asset(
//             &mesh_label(&gltf_mesh),
//             LoadedAsset::new(super::GltfMesh {
//                 primitives,
//                 extras: get_gltf_extras(gltf_mesh.extras()),
//             }),
//         );
//         if let Some(name) = gltf_mesh.name() {
//             named_meshes.insert(name.to_string(), handle.clone());
//         }
//         meshes.push(handle);
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
// fn load_material(
//     material: &gltf::Material,
//     load_context: &mut LoadContext,
// ) -> Handle<crate::Material> {
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
//         LoadedAsset::new(crate::Material {
//             base_color: vec3(color[0], color[1], color[2]),
//             base_color_texture,
//             perceptual_roughness: pbr.roughness_factor(),
//             metallic: pbr.metallic_factor(),
//             metallic_roughness_texture,
//             normal_map_texture,
//             double_sided: material.double_sided(),
//             cull_mode: if material.double_sided() {
//                 None
//             } else {
//                 Some(CullModeFlags::BACK)
//             },
//             occlusion_texture,
//             emissive: vec3(emissive[0], emissive[1], emissive[2]),
//             emissive_texture,
//             unlit: material.unlit(),
//             alpha_mode: alpha_mode(material),
//             ..Default::default()
//         }),
//     )
// }

// /// Returns the label for the `mesh`.
// fn mesh_label(mesh: &gltf::Mesh) -> String {
//     format!("Mesh{}", mesh.index())
// }

// /// Returns the label for the `mesh` and `primitive`.
// fn primitive_label(mesh: &gltf::Mesh, primitive: &Primitive) -> String {
//     format!("Mesh{}/Primitive{}", mesh.index(), primitive.index())
// }

// fn primitive_name(mesh: &gltf::Mesh, primitive: &Primitive) -> String {
//     let mesh_name = mesh.name().unwrap_or("Mesh");
//     if mesh.primitives().len() > 1 {
//         format!("{}.{}", mesh_name, primitive.index())
//     } else {
//         mesh_name.to_string()
//     }
// }

// /// Returns the label for the morph target of `primitive`.
// fn morph_targets_label(mesh: &gltf::Mesh, primitive: &Primitive) -> String {
//     format!(
//         "Mesh{}/Primitive{}/MorphTargets",
//         mesh.index(),
//         primitive.index()
//     )
// }

// /// Returns the label for the `material`.
// fn material_label(material: &gltf::Material) -> String {
//     if let Some(index) = material.index() {
//         format!("Material{index}")
//     } else {
//         "MaterialDefault".to_string()
//     }
// }

// /// Returns the label for the `texture`.
// fn texture_label(texture: &gltf::Texture) -> String {
//     format!("Texture{}", texture.index())
// }

// /// Returns the label for the `node`.
// fn node_label(node: &gltf::Node) -> String {
//     format!("Node{}", node.index())
// }

// /// Returns the label for the `scene`.
// fn scene_label(scene: &gltf::Scene) -> String {
//     format!("Scene{}", scene.index())
// }

// fn skin_label(skin: &gltf::Skin) -> String {
//     format!("Skin{}", skin.index())
// }

// fn alpha_mode(material: &gltf::Material) -> AlphaMode {
//     match material.alpha_mode() {
//         gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
//         gltf::material::AlphaMode::Mask => AlphaMode::Mask(material.alpha_cutoff().unwrap_or(0.5)),
//         gltf::material::AlphaMode::Blend => AlphaMode::Blend,
//     }
// }

// /// Maps the `primitive_topology` form glTF to `wgpu`.
// fn get_primitive_topology(mode: Mode) -> Result<PrimitiveTopology, GltfError> {
//     match mode {
//         Mode::Points => Ok(PrimitiveTopology::POINT_LIST),
//         Mode::Lines => Ok(PrimitiveTopology::LINE_LIST),
//         Mode::LineStrip => Ok(PrimitiveTopology::LINE_STRIP),
//         Mode::Triangles => Ok(PrimitiveTopology::TRIANGLE_LIST),
//         Mode::TriangleStrip => Ok(PrimitiveTopology::TRIANGLE_STRIP),
//         mode => Err(GltfError::UnsupportedPrimitive { mode }),
//     }
// }

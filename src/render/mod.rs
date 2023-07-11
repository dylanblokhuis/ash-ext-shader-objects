pub mod bundles;
pub mod extract;
pub mod global_descriptors;
pub mod gltf;
pub mod image;
pub mod material;
pub mod mesh;
pub mod nodes;
pub mod pipeline;
pub mod primitives;
pub mod shaders;

use std::{
    collections::{BTreeMap, HashMap},
    mem::size_of,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use ash::vk::{
    self, DescriptorImageInfo, ImageCreateInfo, PrimitiveTopology, VertexInputAttributeDescription,
    VertexInputAttributeDescription2EXT, VertexInputBindingDescription,
    VertexInputBindingDescription2EXT, VertexInputRate,
};
use bevy::{
    app::{AppExit, AppLabel, SubApp},
    asset::HandleId,
    ecs::{event::ManualEventReader, schedule::ScheduleLabel, system::SystemState},
    prelude::*,
    time::{create_time_channels, TimeSender},
    utils::Instant,
    window::{PrimaryWindow, RawHandleWrapper},
};
use bytemuck::offset_of;
use gpu_allocator::{
    vulkan::{Allocator, AllocatorCreateDesc},
    MemoryLocation,
};

use crate::{buffer::Buffer, ctx::ExampleBase};

use self::{
    bundles::{Camera, MaterialMeshBundle},
    extract::Extract,
    global_descriptors::GlobalDescriptorSet,
    image::Image,
    material::{Material, MaterialUniform},
    mesh::Mesh,
    nodes::PresentNode,
};

/// Contains the default Bevy rendering backend based on wgpu.
#[derive(Default)]
pub struct RenderPlugin {}

/// The labels of the default App rendering sets.
///
/// The sets run in the order listed, with [`apply_system_buffers`] inserted between each set.
///
/// The `*Flush` sets are assigned to the copy of [`apply_system_buffers`]
/// that runs immediately after the matching system set.
/// These can be useful for ordering, but you almost never want to add your systems to these sets.
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum RenderSet {
    /// The copy of [`apply_system_buffers`] that runs at the beginning of this schedule.
    /// This is used for applying the commands from the [`ExtractSchedule`]
    ExtractCommands,
    /// Prepare render resources from the extracted data for the GPU.
    Prepare,
    /// The copy of [`apply_system_buffers`] that runs immediately after `Prepare`.
    PrepareFlush,
    /// Create [`BindGroups`](crate::render_resource::BindGroup) that depend on
    /// [`Prepare`](RenderSet::Prepare) data and queue up draw calls to run during the
    /// [`Render`](RenderSet::Render) step.
    Queue,
    /// The copy of [`apply_system_buffers`] that runs immediately after `Queue`.
    QueueFlush,
    // TODO: This could probably be moved in favor of a system ordering abstraction in Render or Queue
    /// Sort the [`RenderPhases`](crate::render_phase::RenderPhase) here.
    PhaseSort,
    /// The copy of [`apply_system_buffers`] that runs immediately after `PhaseSort`.
    PhaseSortFlush,
    /// Actual rendering happens here.
    /// In most cases, only the render backend should insert resources here.
    Render,
    /// The copy of [`apply_system_buffers`] that runs immediately after `Render`.
    RenderFlush,
    /// Cleanup render resources here.
    Cleanup,
    /// The copy of [`apply_system_buffers`] that runs immediately after `Cleanup`.
    CleanupFlush,
}

#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
pub struct Render;

impl Render {
    /// Sets up the base structure of the rendering [`Schedule`].
    ///
    /// The sets defined in this enum are configured to run in order,
    /// and a copy of [`apply_deferred`] is inserted into each `*Flush` set.
    pub fn base_schedule() -> Schedule {
        use RenderSet::*;

        let mut schedule = Schedule::new();

        // Create "stage-like" structure using buffer flushes + ordering
        schedule.add_systems((
            apply_deferred.in_set(PrepareFlush),
            apply_deferred.in_set(QueueFlush),
            apply_deferred.in_set(PhaseSortFlush),
            apply_deferred.in_set(RenderFlush),
            apply_deferred.in_set(CleanupFlush),
        ));
        schedule.configure_sets(
            (
                ExtractCommands,
                Prepare,
                PrepareFlush,
                Queue,
                QueueFlush,
                PhaseSort,
                PhaseSortFlush,
                Render,
                RenderFlush,
                Cleanup,
                CleanupFlush,
            )
                .chain(),
        );

        schedule
    }
}

/// Schedule which extract data from the main world and inserts it into the render world.
///
/// This step should be kept as short as possible to increase the "pipelining potential" for
/// running the next frame while rendering the current frame.
///
/// This schedule is run on the main world, but its buffers are not applied
/// via [`Schedule::apply_system_buffers`](bevy_ecs::schedule::Schedule) until it is returned to the render world.
#[derive(ScheduleLabel, PartialEq, Eq, Debug, Clone, Hash)]
pub struct ExtractSchedule;

/// The simulation [`World`] of the application, stored as a resource.
/// This resource is only available during [`ExtractSchedule`] and not
/// during command application of that schedule.
/// See [`Extract`] for more details.
#[derive(Resource, Default)]
pub struct MainWorld(World);

impl Deref for MainWorld {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MainWorld {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource, Default)]
pub struct NonSendMarker;

/// A Label for the rendering sub-app.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, AppLabel)]
pub struct RenderApp;

impl Plugin for RenderPlugin {
    fn build(&self, _app: &mut App) {}

    fn ready(&self, app: &App) -> bool {
        app.world.components().iter().find(|c| c.name() == "bevy_window::raw_handle::RawHandleWrapper").is_some()
    }

    /// Initializes the renderer, sets up the [`RenderSet`](RenderSet) and creates the rendering sub-app.
    fn finish(&self, app: &mut App) {
        app.init_resource::<ScratchMainWorld>()
            .add_asset::<Mesh>()
            .add_asset::<Material>()
            .add_asset::<crate::render::image::Image>()
            .add_asset_loader(crate::render::image::ImageTextureLoader);

        let mut system_state: SystemState<
            Query<(&RawHandleWrapper, &Window), With<PrimaryWindow>>,
        > = SystemState::new(&mut app.world);
        let window_query = system_state.get(&app.world);
        let (window_handle, window) = window_query.get_single().unwrap();
        let render_instance = RenderInstance(Arc::new(ExampleBase::new(
            window_handle,
            window.present_mode,
        )));

        let render_allocator = RenderAllocator(
            Allocator::new(&AllocatorCreateDesc {
                instance: render_instance.0.instance.clone(),
                device: render_instance.0.device.clone(),
                physical_device: render_instance.0.pdevice,
                debug_settings: Default::default(),
                buffer_device_address: true, // Ideally, check the BufferDeviceAddressFeatures struct.
                allocation_sizes: Default::default(),
            })
            .unwrap(),
        );
        let global_descriptor_set = GlobalDescriptorSet::new(&render_instance);

        let mut render_app = App::empty();
        render_app.main_schedule_label = Box::new(Render);

        let mut extract_schedule = Schedule::new();
        extract_schedule.set_apply_final_deferred(false);

        render_app
            .add_schedule(ExtractSchedule, extract_schedule)
            .add_schedule(Render, Render::base_schedule())
            .add_systems(
                Render,
                (
                    apply_extract_commands.in_set(RenderSet::ExtractCommands),
                    render_system.in_set(RenderSet::Render),
                ),
            )
            .init_non_send_resource::<NonSendMarker>()
            .init_resource::<ProcessedRenderAssets>()
            .init_resource::<SequentialPassSystem>()
            .insert_resource(render_instance)
            .insert_resource(render_allocator)
            .insert_resource(global_descriptor_set)
            .add_systems(ExtractSchedule, extract_meshes)
            .add_systems(ExtractSchedule, extract_materials)
            .add_systems(ExtractSchedule, extract_camera_uniform)
            .add_systems(ExtractSchedule, extract_objects)
            .add_systems(ExtractSchedule, extract_textures_from_materials)
            .add_systems(Render, basic_renderer_setup.in_set(RenderSet::Prepare));

        let (sender, receiver) = create_time_channels();
        app.insert_resource(receiver);
        render_app.insert_resource(sender);

        app.insert_sub_app(
            RenderApp,
            SubApp::new(render_app, move |main_world, render_app| {
                // reserve all existing main world entities for use in render_app
                // they can only be spawned using `get_or_spawn()`
                // let total_count = main_world.entities().total_count();

                // assert_eq!(
                //     render_app.world.entities().len(),
                //     0,
                //     "An entity was spawned after the entity list was cleared last frame and before the extract schedule began. This is not supported",
                // );

                // // This is safe given the clear_entities call in the past frame and the assert above
                // unsafe {
                //     render_app
                //         .world
                //         .entities_mut()
                //         .flush_and_reserve_invalid_assuming_no_entities(total_count);
                // }

                // run extract schedule
                extract(main_world, render_app);
            }),
        );
    }
}

/// A "scratch" world used to avoid allocating new worlds every frame when
/// swapping out the [`MainWorld`] for [`ExtractSchedule`].
#[derive(Resource, Default)]
struct ScratchMainWorld(World);

/// Executes the [`ExtractSchedule`] step of the renderer.
/// This updates the render world with the extracted ECS data of the current frame.
fn extract(main_world: &mut World, render_app: &mut App) {
    // temporarily add the app world to the render world as a resource
    let scratch_world = main_world.remove_resource::<ScratchMainWorld>().unwrap();
    let inserted_world = std::mem::replace(main_world, scratch_world.0);
    render_app.world.insert_resource(MainWorld(inserted_world));

    render_app.world.run_schedule(ExtractSchedule);

    // move the app world back, as if nothing happened.
    let inserted_world = render_app.world.remove_resource::<MainWorld>().unwrap();
    let scratch_world = std::mem::replace(main_world, inserted_world.0);
    main_world.insert_resource(ScratchMainWorld(scratch_world));
}

/// Applies the commands from the extract schedule. This happens during
/// the render schedule rather than during extraction to allow the commands to run in parallel with the
/// main app when pipelined rendering is enabled.
fn apply_extract_commands(render_world: &mut World) {
    render_world.resource_scope(|render_world, mut schedules: Mut<Schedules>| {
        schedules
            .get_mut(&ExtractSchedule)
            .unwrap()
            .apply_deferred(render_world);
    });
}

pub trait SequentialNode: Send + Sync + 'static {
    /// Updates internal node state using the current render [`World`] prior to the run method.
    fn update(&mut self, _world: &mut World) {}

    fn run(&self, world: &mut World) -> anyhow::Result<()>;
}

struct SequentialPass {
    pub id: String,
    pub node: Box<dyn SequentialNode>,
}

#[derive(Default, Resource)]
struct SequentialPassSystem {
    passes: Vec<SequentialPass>,
}

impl SequentialPassSystem {
    pub fn add_pass(&mut self, id: String, node: Box<dyn SequentialNode>) {
        self.passes.push(SequentialPass { id, node });
    }

    pub fn remove_pass(&mut self, id: &str) {
        self.passes.retain(|pass| pass.id != id);
    }

    pub fn get_pass(&self, id: &str) -> Option<&SequentialPass> {
        self.passes.iter().find(|pass| pass.id == id)
    }

    pub fn update(&mut self, world: &mut World) {
        for pass in self.passes.iter_mut() {
            pass.node.update(world);
        }
    }

    pub fn run(&mut self, world: &mut World) {
        for pass in self.passes.iter_mut() {
            pass.node.run(world).unwrap();
        }
    }
}

/**
 * This runs after all the extraction has been done
 */
fn render_system(world: &mut World) {
    world.resource_scope(|world, mut graph: Mut<SequentialPassSystem>| {
        graph.update(world);
        graph.run(world);
    });

    // update the time and send it to the app world
    let time_sender = world.resource::<TimeSender>();
    time_sender.0.try_send(Instant::now()).expect(
        "The TimeSender channel should always be empty during render. You might need to add the bevy::core::time_system to your app.",
    );
}

#[derive(Resource)]
pub struct RenderInstance(pub Arc<ExampleBase>);
impl RenderInstance {
    pub fn device(&self) -> &ash::Device {
        &self.0.device
    }
}

#[derive(Resource)]
pub struct RenderAllocator(Allocator);
impl RenderAllocator {
    pub fn allocator(&mut self) -> &mut Allocator {
        &mut self.0
    }
}

#[derive(Debug)]
struct GpuMesh {
    vertex_buffer: Buffer,
    index_buffer: Option<Buffer>,
    vertex_count: u32,
    index_count: u32,
    topology: PrimitiveTopology,
}

impl GpuMesh {
    pub fn vertex_binding_descriptors() -> VertexInputBindingDescription {
        VertexInputBindingDescription::default()
            .binding(0)
            .input_rate(VertexInputRate::VERTEX)
            .stride(std::mem::size_of::<mesh::Vertex>() as u32)
    }

    pub fn vertex_binding_descriptors2() -> VertexInputBindingDescription2EXT<'static> {
        VertexInputBindingDescription2EXT::default()
            .binding(0)
            .input_rate(VertexInputRate::VERTEX)
            .divisor(1)
            .stride(std::mem::size_of::<mesh::Vertex>() as u32)
    }

    pub fn vertex_input_descriptors() -> [vk::VertexInputAttributeDescription; 5] {
        return [
            VertexInputAttributeDescription::default()
                .binding(0)
                .location(0)
                .format(ash::vk::Format::R32G32B32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, position) as u32),
            VertexInputAttributeDescription::default()
                .binding(0)
                .location(1)
                .format(ash::vk::Format::R32G32B32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, normal) as u32),
            VertexInputAttributeDescription::default()
                .binding(0)
                .location(2)
                .format(ash::vk::Format::R32G32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, uv) as u32),
            VertexInputAttributeDescription::default()
                .binding(0)
                .location(3)
                .format(ash::vk::Format::R32G32B32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, tangent) as u32),
            VertexInputAttributeDescription::default()
                .binding(0)
                .location(4)
                .format(ash::vk::Format::R32G32B32A32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, color) as u32),
        ];
    }

    pub fn vertex_input_descriptors2() -> [vk::VertexInputAttributeDescription2EXT<'static>; 5] {
        return [
            VertexInputAttributeDescription2EXT::default()
                .binding(0)
                .location(0)
                .format(ash::vk::Format::R32G32B32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, position) as u32),
            VertexInputAttributeDescription2EXT::default()
                .binding(0)
                .location(1)
                .format(ash::vk::Format::R32G32B32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, normal) as u32),
            VertexInputAttributeDescription2EXT::default()
                .binding(0)
                .location(2)
                .format(ash::vk::Format::R32G32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, uv) as u32),
            VertexInputAttributeDescription2EXT::default()
                .binding(0)
                .location(3)
                .format(ash::vk::Format::R32G32B32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, tangent) as u32),
            VertexInputAttributeDescription2EXT::default()
                .binding(0)
                .location(4)
                .format(ash::vk::Format::R32G32B32A32_SFLOAT)
                .offset(offset_of!(mesh::Vertex, color) as u32),
        ];
    }
}

#[derive(Resource, Default)]
struct ProcessedRenderAssets {
    meshes: HashMap<Handle<Mesh>, GpuMesh>,
}

fn extract_meshes(
    objects_with_mesh: Extract<Query<&Handle<Mesh>, Changed<Handle<Mesh>>>>,
    mesh_assets: Extract<Res<Assets<Mesh>>>,
    render_instance: Res<RenderInstance>,
    mut render_allocator: ResMut<RenderAllocator>,
    mut processed_assets: ResMut<ProcessedRenderAssets>,
) {
    for mesh_handle in objects_with_mesh.iter() {
        let _ = info_span!("Extracting mesh").entered();
        // if processed_assets.meshes.contains_key(mesh_handle) {
        //     continue;
        // }
        let mesh = mesh_assets.get(mesh_handle).unwrap();
        let vertex_buffer = {
            let mut buf = Buffer::new(
                &render_instance.0.device,
                &mut render_allocator.0,
                &vk::BufferCreateInfo {
                    size: mesh.vertices.len() as u64 * std::mem::size_of::<mesh::Vertex>() as u64,
                    usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                    sharing_mode: vk::SharingMode::EXCLUSIVE,
                    ..Default::default()
                },
                MemoryLocation::CpuToGpu,
            );

            buf.copy_from_slice(&mesh.vertices, 0);
            buf
        };

        let (index_buffer, index_len) = || -> (Option<Buffer>, u32) {
            if mesh.indices.is_empty() {
                return (None, 0);
            }
            let mut buf = Buffer::new(
                &render_instance.0.device,
                &mut render_allocator.0,
                &vk::BufferCreateInfo::default()
                    .size((size_of::<u32>() * mesh.indices.len()) as vk::DeviceSize)
                    .usage(vk::BufferUsageFlags::INDEX_BUFFER)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                MemoryLocation::CpuToGpu,
            );

            buf.copy_from_slice(&mesh.indices, 0);
            (Some(buf), mesh.indices.len() as u32)
        }();

        processed_assets.meshes.insert(
            mesh_handle.clone(),
            GpuMesh {
                vertex_buffer,
                index_buffer,
                vertex_count: mesh.vertices.len() as u32,
                index_count: index_len,
                topology: mesh.primitive_topology,
            },
        );
    }

    // cleanup old meshes and delete gpu buffers
    // let mut keys_to_delete = vec![];
    // for (handle, gpu_mesh) in processed_assets.meshes.iter_mut() {
    //     if !objects_with_mesh.into_iter().any(|h| h.0 == handle) {
    //         gpu_mesh
    //             .vertex_buffer
    //             .destroy(render_instance.device(), render_allocator.allocator());

    //         if let Some(index_buffer) = &mut gpu_mesh.index_buffer {
    //             index_buffer.destroy(render_instance.device(), render_allocator.allocator());
    //         }

    //         keys_to_delete.push(handle.clone());
    //     }
    // }

    // for i in keys_to_delete.iter().rev() {
    //     processed_assets.meshes.remove(i);
    // }
}

fn extract_objects(
    mut commands: Commands,
    objects: Extract<
        Query<(Entity, &Handle<Mesh>, &Handle<Material>, &Transform), Changed<Handle<Mesh>>>,
    >,
) {
    if objects.iter().count() == 0 {
        return;
    }
    let _ = info_span!("Extracting objects").entered();
    let mut values = Vec::new();
    for (entity, mesh_handle, material_handle, transform) in objects.iter() {
        values.push((
            entity,
            MaterialMeshBundle {
                mesh: mesh_handle.clone(),
                material: material_handle.clone(),
                transform: *transform,
            },
        ));
    }
    commands.insert_or_spawn_batch(values);
}

fn extract_textures_from_materials(
    material_assets: Extract<Res<Assets<Material>>>,
    texture_assets: Extract<Res<Assets<Image>>>,
    mut ev_asset: Extract<EventReader<AssetEvent<Image>>>,
    render_instance: Res<RenderInstance>,
    mut render_allocator: ResMut<RenderAllocator>,
    mut global_descriptors: ResMut<GlobalDescriptorSet>,
) {
    for ev in ev_asset.iter() {
        match ev {
            AssetEvent::Created { handle } => {
                let material = material_assets
                    .iter()
                    .map(|(material_handle_id, material)| {
                        if let Some(base_color_texture) = material.base_color_texture.as_ref() {
                            if base_color_texture == handle {
                                return Some((
                                    material_handle_id,
                                    base_color_texture,
                                    offset_of!(MaterialUniform, base_color_texture_index),
                                ));
                            }
                        }

                        if let Some(emissive_texture) = material.emissive_texture.as_ref() {
                            if emissive_texture == handle {
                                return Some((
                                    material_handle_id,
                                    emissive_texture,
                                    offset_of!(MaterialUniform, emissive_texture_index),
                                ));
                            }
                        }

                        if let Some(occlusion_texture) = material.occlusion_texture.as_ref() {
                            if occlusion_texture == handle {
                                return Some((
                                    material_handle_id,
                                    occlusion_texture,
                                    offset_of!(MaterialUniform, occlusion_texture_index),
                                ));
                            }
                        }

                        if let Some(normal_map_texture) = material.normal_map_texture.as_ref() {
                            if normal_map_texture == handle {
                                return Some((
                                    material_handle_id,
                                    normal_map_texture,
                                    offset_of!(MaterialUniform, normal_map_texture_index),
                                ));
                            }
                        }

                        if let Some(metallic_roughness_texture) =
                            material.metallic_roughness_texture.as_ref()
                        {
                            if metallic_roughness_texture == handle {
                                return Some((
                                    material_handle_id,
                                    metallic_roughness_texture,
                                    offset_of!(MaterialUniform, metallic_roughness_texture_index),
                                ));
                            }
                        }

                        None
                    })
                    .find(|x| x.is_some())
                    .flatten();

                let Some((material_handle_id, texture_handle, bytes_offset))  = material else {
                    continue;
                };

                let texture = texture_assets.get(texture_handle).unwrap();
                global_descriptors.textures.insert(
                    texture_handle.clone(),
                    crate::buffer::Image::from_image_buffer(
                        &render_instance,
                        &mut render_allocator,
                        texture.data.clone(),
                        texture.format,
                    ),
                );
                let index = global_descriptors
                    .get_texture_index(texture_handle)
                    .unwrap() as i32;

                if let Some(buffer) = global_descriptors.buffers.get_mut(&material_handle_id) {
                    buffer.copy_from_slice(&[index], bytes_offset);
                } else {
                    let mut buffer: Buffer = Buffer::new(
                        render_instance.device(),
                        render_allocator.allocator(),
                        &vk::BufferCreateInfo::default()
                            .size(std::mem::size_of::<material::MaterialUniform>() as u64)
                            .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                            .sharing_mode(vk::SharingMode::EXCLUSIVE),
                        MemoryLocation::CpuToGpu,
                    );
                    buffer.copy_from_slice(&[index], bytes_offset);
                    global_descriptors
                        .buffers
                        .insert(material_handle_id, buffer);
                }
            }
            AssetEvent::Modified { handle } => {
                // an image was modified
            }
            AssetEvent::Removed { handle } => {
                // an image was unloaded
            }
        }
    }
}

fn extract_materials(
    materials: Extract<Query<&Handle<Material>, Changed<Handle<Material>>>>,
    material_assets: Extract<Res<Assets<Material>>>,
    texture_assets: Extract<Res<Assets<Image>>>,
    render_instance: Res<RenderInstance>,
    mut render_allocator: ResMut<RenderAllocator>,
    mut global_descriptors: ResMut<GlobalDescriptorSet>,
) {
    for handle in materials.iter() {
        let _ = info_span!("Extracting material").entered();
        let material = material_assets.get(handle).unwrap();
        let mut material_buffer = MaterialUniform::from_material(material);

        if let Some(handle) = material.base_color_texture.as_ref() {
            if let Some(img) = texture_assets.get(handle) {
                let mut texture = crate::buffer::Image::from_image_buffer(
                    &render_instance,
                    &mut render_allocator,
                    img.data.clone(),
                    img.format,
                );

                let _ = texture.create_view(render_instance.device());
                global_descriptors.textures.insert(handle.clone(), texture);
                material_buffer.base_color_texture_index =
                    global_descriptors.get_texture_index(handle).unwrap() as i32;
            }
        }

        if let Some(buffer) = global_descriptors.buffers.get_mut(&handle.id()) {
            buffer.copy_from_slice(&[material_buffer], 0);
        } else {
            let buffer = {
                let mut buf = Buffer::new(
                    render_instance.device(),
                    render_allocator.allocator(),
                    &vk::BufferCreateInfo {
                        size: std::mem::size_of::<material::MaterialUniform>() as u64,
                        usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
                        sharing_mode: vk::SharingMode::EXCLUSIVE,
                        ..Default::default()
                    },
                    MemoryLocation::CpuToGpu,
                );

                buf.copy_from_slice(&[material_buffer], 0);
                buf
            };

            global_descriptors.buffers.insert(handle.id(), buffer);
        }
    }
}

#[repr(C, align(16))]
#[derive(Copy, Clone, Debug)]
struct CameraBuffer {
    view_proj: Mat4,
    inverse_view_proj: Mat4,
    view: Mat4,
    inverse_view: Mat4,
    proj: Mat4,
    inverse_proj: Mat4,
    world_position: Vec3,
}
pub static CAMERA_HANDLE: once_cell::sync::Lazy<HandleId> =
    once_cell::sync::Lazy::new(|| HandleId::from(String::from("camera")));

/// only runs whenever the camera component or transform component changes
fn extract_camera_uniform(
    camera: Extract<Query<(&Camera, &Transform), Or<(Changed<Camera>, Changed<Transform>)>>>,
    mut global_descriptor_set: ResMut<GlobalDescriptorSet>,
    render_instance: Res<RenderInstance>,
    mut render_allocator: ResMut<RenderAllocator>,
) {
    let Ok((camera, camera_transform)) = camera.get_single() else {
        return;
    };
    let _ = info_span!("Extracting camera uniform").entered();

    let view = camera_transform.compute_matrix();
    let inverse_view = view.inverse();
    let projection = camera.projection;
    let inverse_projection = projection.inverse();

    let uniform = CameraBuffer {
        view_proj: projection * inverse_view,
        inverse_view_proj: view * inverse_projection,
        view,
        inverse_view,
        proj: projection,
        inverse_proj: inverse_projection,
        world_position: camera_transform.translation,
    };

    if let Some(buffer) = global_descriptor_set.buffers.get_mut(&CAMERA_HANDLE) {
        buffer.copy_from_slice(&[uniform], 0);
    } else {
        let mut buffer: Buffer = Buffer::new(
            render_instance.device(),
            render_allocator.allocator(),
            &vk::BufferCreateInfo::default()
                .size(std::mem::size_of::<CameraBuffer>() as u64)
                .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE),
            MemoryLocation::CpuToGpu,
        );
        buffer.copy_from_slice(&[uniform], 0);
        global_descriptor_set.buffers.insert(*CAMERA_HANDLE, buffer);
    }
}

fn basic_renderer_setup(
    mut sequential_pass_system: ResMut<SequentialPassSystem>,
    render_instance: Res<RenderInstance>,
    mut render_allocator: ResMut<RenderAllocator>,
) {
    if !sequential_pass_system.passes.is_empty() {
        return;
    }

    sequential_pass_system.add_pass(
        "present_node".into(),
        Box::new(PresentNode::new(&render_instance, &mut render_allocator)),
    );
}

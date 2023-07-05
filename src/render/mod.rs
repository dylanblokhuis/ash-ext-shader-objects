pub mod bundles;
pub mod extract;
pub mod gltf;
pub mod image;
pub mod material;
pub mod mesh;
pub mod nodes;
pub mod primitives;
pub mod shaders;

use std::{
    collections::HashMap,
    mem::size_of,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use ash::vk::{
    self, ImageCreateInfo, PrimitiveTopology, VertexInputAttributeDescription2EXT,
    VertexInputBindingDescription2EXT, VertexInputRate,
};
use bevy::{
    app::{AppExit, AppLabel, SubApp},
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

impl RenderSet {
    /// Sets up the base structure of the rendering [`Schedule`].
    ///
    /// The sets defined in this enum are configured to run in order,
    /// and a copy of [`apply_system_buffers`] is inserted into each `*Flush` set.
    pub fn base_schedule() -> Schedule {
        use RenderSet::*;

        let mut schedule = Schedule::new();

        // Create "stage-like" structure using buffer flushes + ordering
        schedule.add_system(apply_system_buffers.in_set(PrepareFlush));
        schedule.add_system(apply_system_buffers.in_set(QueueFlush));
        schedule.add_system(apply_system_buffers.in_set(PhaseSortFlush));
        schedule.add_system(apply_system_buffers.in_set(RenderFlush));
        schedule.add_system(apply_system_buffers.in_set(CleanupFlush));

        schedule.configure_set(ExtractCommands.before(Prepare));
        schedule.configure_set(Prepare.after(ExtractCommands).before(PrepareFlush));
        schedule.configure_set(Queue.after(PrepareFlush).before(QueueFlush));
        schedule.configure_set(PhaseSort.after(QueueFlush).before(PhaseSortFlush));
        schedule.configure_set(Render.after(PhaseSortFlush).before(RenderFlush));
        schedule.configure_set(Cleanup.after(RenderFlush).before(CleanupFlush));

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
    /// Initializes the renderer, sets up the [`RenderSet`](RenderSet) and creates the rendering sub-app.
    fn build(&self, app: &mut App) {
        app.init_resource::<ScratchMainWorld>()
            .add_asset::<Mesh>()
            .add_asset::<Material>()
            .add_asset::<crate::render::image::Image>()
            .add_system(cleanup_on_exit);

        let mut system_state: SystemState<Query<&RawHandleWrapper, With<PrimaryWindow>>> =
            SystemState::new(&mut app.world);
        let primary_window = system_state.get(&app.world).get_single().ok().cloned();

        let render_instance = primary_window
            .map(|wrapper| {
                RenderInstance(Arc::new(ExampleBase::new(
                    wrapper,
                    bevy::window::PresentMode::AutoVsync,
                )))
            })
            .unwrap();

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

        let mut render_app = App::empty();
        render_app.add_simple_outer_schedule();
        let mut render_schedule = RenderSet::base_schedule();

        // Prepare the schedule which extracts data from the main world to the render world
        render_app.edit_schedule(ExtractSchedule, |schedule| {
            schedule.set_apply_final_buffers(false);
        });

        // This set applies the commands from the extract stage while the render schedule
        // is running in parallel with the main app.
        render_schedule.add_system(apply_extract_commands.in_set(RenderSet::ExtractCommands));
        render_schedule.add_system(render_system.in_set(RenderSet::Render));
        render_schedule.add_system(World::clear_entities.in_set(RenderSet::Cleanup));

        render_app
            .init_non_send_resource::<NonSendMarker>()
            .init_resource::<ProcessedRenderAssets>()
            .init_resource::<SequentialPassSystem>()
            .insert_resource(render_instance)
            .insert_resource(render_allocator)
            .add_schedule(CoreSchedule::Main, render_schedule)
            // .add_system(extract_render_instance.in_schedule(ExtractSchedule))
            .add_system(extract_meshes.in_schedule(ExtractSchedule))
            .add_system(extract_materials.in_schedule(ExtractSchedule))
            .add_system(extract_camera_uniform.in_schedule(ExtractSchedule))
            .add_system(extract_objects.in_schedule(ExtractSchedule))
            .add_system(basic_renderer_setup.in_set(RenderSet::Prepare));

        let (sender, receiver) = create_time_channels();
        app.insert_resource(receiver);
        render_app.insert_resource(sender);

        app.insert_sub_app(RenderApp, SubApp::new(render_app, move |main_world, render_app| {
            // reserve all existing main world entities for use in render_app
            // they can only be spawned using `get_or_spawn()`
            let total_count = main_world.entities().total_count();

            assert_eq!(
                render_app.world.entities().len(),
                0,
                "An entity was spawned after the entity list was cleared last frame and before the extract schedule began. This is not supported",
            );

            // This is safe given the clear_entities call in the past frame and the assert above
            unsafe {
                render_app
                    .world
                    .entities_mut()
                    .flush_and_reserve_invalid_assuming_no_entities(total_count);
            }

        // run extract schedule
        extract(main_world, render_app);
    }));
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
            .apply_system_buffers(render_world);
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

// fn extract_render_instance(
//     mut commands: Commands,
//     _marker: NonSend<NonSendMarker>,
//     windows: Extract<Query<(Entity, &Window, &RawHandleWrapper, Option<&PrimaryWindow>)>>,
//     vulkano_windows: Extract<NonSend<BevyVulkanoWindows>>,
//     setup: Res<RenderResourcesSetup>,
// ) {
//     if setup.0 {
//         return;
//     }
//     let Ok((entity, _, _, _)) = windows.get_single() else {
//         return;
//     };
//     let Some(vulkano_window) = vulkano_windows.get_vulkano_window(entity) else {
//         return;
//     };

//     let renderer = vulkano_window.renderer.clone();
//     let allocator = Allocator::new(&AllocatorCreateDesc {
//         instance: renderer.instance.clone(),
//         device: renderer.device.clone(),
//         physical_device: renderer.pdevice,
//         debug_settings: Default::default(),
//         buffer_device_address: true, // Ideally, check the BufferDeviceAddressFeatures struct.
//         allocation_sizes: Default::default(),
//     })
//     .unwrap();
//     commands.insert_resource(RenderAllocator(allocator));
//     commands.insert_resource(RenderInstance(vulkano_window.renderer.clone()));
//     commands.insert_resource(RenderResourcesSetup(true))
// }

#[derive(Debug)]
struct GpuMesh {
    vertex_buffer: Buffer,
    index_buffer: Option<Buffer>,
    vertex_count: u32,
    index_count: u32,
    topology: PrimitiveTopology,
}

impl GpuMesh {
    pub fn vertex_binding_descriptors() -> VertexInputBindingDescription2EXT<'static> {
        VertexInputBindingDescription2EXT::default()
            .binding(0)
            .input_rate(VertexInputRate::VERTEX)
            .divisor(1)
            .stride(std::mem::size_of::<mesh::Vertex>() as u32)
    }

    pub fn vertex_input_descriptors() -> [vk::VertexInputAttributeDescription2EXT<'static>; 5] {
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
    materials: HashMap<Handle<Material>, GpuMaterial>,
    buffers: HashMap<&'static str, Buffer>,
    textures: HashMap<Handle<Image>, crate::buffer::Image>,
}

#[tracing::instrument(skip_all)]
fn extract_meshes(
    objects_with_mesh: Extract<Query<&Handle<Mesh>>>,
    mesh_assets: Extract<Res<Assets<Mesh>>>,
    render_instance: Res<RenderInstance>,
    mut render_allocator: ResMut<RenderAllocator>,
    mut processed_assets: ResMut<ProcessedRenderAssets>,
) {
    for mesh_handle in objects_with_mesh.iter() {
        if processed_assets.meshes.contains_key(mesh_handle) {
            continue;
        }
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

#[tracing::instrument(skip_all)]
fn extract_objects(
    mut commands: Commands,
    objects: Extract<Query<(Entity, &Handle<Mesh>, &Handle<Material>, &Transform)>>,
) {
    for (entity, mesh_handle, material_handle, transform) in objects.iter() {
        commands.get_or_spawn(entity).insert(MaterialMeshBundle {
            mesh: mesh_handle.clone(),
            material: material_handle.clone(),
            transform: *transform,
        });
    }
}

struct GpuMaterial {
    material_buffer: Buffer,
}
#[tracing::instrument(skip_all)]
fn extract_materials(
    materials: Extract<Query<&Handle<Material>>>,
    material_assets: Extract<Res<Assets<Material>>>,
    texture_assets: Extract<Res<Assets<Image>>>,
    render_instance: Res<RenderInstance>,
    mut render_allocator: ResMut<RenderAllocator>,
    mut processed_assets: ResMut<ProcessedRenderAssets>,
) {
    for handle in materials.iter() {
        if processed_assets.materials.contains_key(handle) {
            continue;
        }

        let material = material_assets.get(handle).unwrap();
        let buffer = {
            let mut buf = Buffer::new(
                &render_instance.0.device,
                &mut render_allocator.0,
                &vk::BufferCreateInfo {
                    size: std::mem::size_of::<material::MaterialUniform>() as u64,
                    usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
                    sharing_mode: vk::SharingMode::EXCLUSIVE,
                    ..Default::default()
                },
                MemoryLocation::CpuToGpu,
            );

            buf.copy_from_slice(&[MaterialUniform::from_material(material.clone())], 0);
            buf
        };

        if let Some(base_color_texture) = material.base_color_texture.as_ref() {
            let img = texture_assets.get(base_color_texture).unwrap();
            let texture = crate::buffer::Image::from_image_buffer(
                &render_instance,
                &mut render_allocator,
                img.data.clone(),
            );
            processed_assets
                .textures
                .insert(base_color_texture.clone(), texture);
        }

        processed_assets.materials.insert(
            handle.clone(),
            GpuMaterial {
                material_buffer: buffer,
            },
        );
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

/// only runs whenever the camera component or transform component changes
#[tracing::instrument(skip_all)]
fn extract_camera_uniform(
    camera: Extract<Query<(&Camera, &Transform), Or<(Changed<Camera>, Changed<Transform>)>>>,
    mut processed_assets: ResMut<ProcessedRenderAssets>,
    render_instance: Res<RenderInstance>,
    mut render_allocator: ResMut<RenderAllocator>,
) {
    let Ok((camera, camera_transform)) = camera.get_single() else {
        return;
    };

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

    if let Some(buffer) = processed_assets.buffers.get_mut("camera") {
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
        processed_assets.buffers.insert("camera", buffer);
    }
}

fn cleanup_on_exit(app_exit_events: Res<Events<AppExit>>) {
    let mut app_exit_event_reader = ManualEventReader::<AppExit>::default();
    if let Some(exit) = app_exit_event_reader.iter(&app_exit_events).last() {
        println!("cleanup!!!!!!!!");
    }
    // for _ in events.iter() {
    //     println!("cleanup!!!!!!!!");
    // }
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

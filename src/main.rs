#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use ash::vk::Filter;
use ash::vk::SamplerAddressMode;
use ash::vk::SamplerMipmapMode;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::diagnostic::LogDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::close_on_esc;
use bevy::window::WindowMode;
use camera_controller::CameraController;
use camera_controller::CameraControllerPlugin;
use ctx::SamplerDesc;
use render::bundles::Camera;
use render::bundles::CameraBundle;
use render::bundles::MaterialMeshBundle;
use render::image::Image;
use render::material::Material;
use render::mesh::Mesh;
use render::primitives;
use render::primitives::Box;
use render::RenderPlugin;
use std::default::Default;

mod buffer;
mod camera_controller;
mod chunky_list;
mod ctx;
mod passes;
mod render;

fn main() {
    #[cfg(feature = "tracing")]
    {
        use tracing_subscriber::layer::SubscriberExt;
        tracing::subscriber::set_global_default(
            tracing_subscriber::registry().with(tracing_tracy::TracyLayer::new()),
        )
        .expect("set up the subscriber");
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1280.0, 720.0).into(),
                title: "Someday".to_string(),
                present_mode: bevy::window::PresentMode::Mailbox,
                resizable: false,
                mode: WindowMode::Windowed,
                ..default()
            }),
            ..default()
        }))
        .add_plugin(RenderPlugin::default())
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin)
        .add_plugin(CameraControllerPlugin)
        .add_system(close_on_esc)
        .add_startup_system(spawn_stuff)
        .run();
}

fn spawn_stuff(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
    mut images: ResMut<Assets<Image>>,
) {
    let _ = info_span!("Spawning objects");
    for x in 0..10 {
        for y in 0..10 {
            commands.spawn(MaterialMeshBundle {
                mesh: meshes.add(Box::new(1.0, 1.0, 1.0).into()),
                material: materials.add(Material {
                    base_color: Vec3::new(x as f32 / 10.0, y as f32 / 10.0, 0.0),
                    ..Default::default()
                }),
                transform: Transform::from_translation(Vec3::new(
                    x as f32 * 2.0,
                    y as f32 * 2.0,
                    x as f32 * 2.0,
                )),
            });
        }
    }

    // commands.spawn(MaterialMeshBundle {
    //     mesh: meshes.add(Box::new(1.0, 1.0, 1.0).into()),
    //     material: materials.add(Material {
    //         base_color: Vec3::new(1.0, 0.0, 0.0),
    //         ..Default::default()
    //     }),
    //     transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
    // });

    // commands.spawn(MaterialMeshBundle {
    //     mesh: meshes.add(Box::new(1.0, 1.0, 1.0).into()),
    //     transform: Transform::from_translation(Vec3::new(5.0, 5.0, 5.0)),
    //     material: materials.add(Material {
    //         base_color: Vec3::new(1.0, 1.0, 0.0),
    //         ..Default::default()
    //     }),
    // });

    commands.spawn(MaterialMeshBundle {
        mesh: meshes.add(primitives::Cube::new(2.0).into()),
        transform: Transform::from_translation(Vec3::new(-10.0, 1.0, 1.0)),
        material: materials.add(Material {
            base_color: Vec3::new(1.0, 0.0, 0.0),
            base_color_texture: Some(images.add(Image {
                data: image::load_from_memory(include_bytes!("../images/public.png")).unwrap(),
                sampler_descriptor: SamplerDesc {
                    texel_filter: Filter::LINEAR,
                    mipmap_mode: SamplerMipmapMode::LINEAR,
                    address_modes: SamplerAddressMode::REPEAT,
                },
            })),
            ..Default::default()
        }),
    });

    // commands.spawn(MaterialMeshBundle {
    //     mesh: meshes.add(Box::new(1.0, 1.0, 1.0).into()),
    //     transform: Transform::from_translation(Vec3::new(-5.0, -5.0, -5.0)),
    // });
    // commands.spawn(MaterialMeshBundle {
    //     mesh: meshes.add(Mesh {
    //         vertices: vec![
    //             Vertex {
    //                 position: [-1.0, 1.0, 0.0],
    //                 color: [0.0, 1.0, 0.0, 1.0],
    //                 ..Default::default()
    //             },
    //             Vertex {
    //                 position: [1.0, 1.0, 0.0],
    //                 color: [0.0, 0.0, 1.0, 1.0],
    //                 ..Default::default()
    //             },
    //             Vertex {
    //                 position: [0.0, -1.0, 0.0],
    //                 color: [1.0, 0.0, 0.0, 1.0],
    //                 ..Default::default()
    //             },
    //         ],
    //         indices: vec![0, 1, 2],
    //         primitive_topology: PrimitiveTopology::TRIANGLE_LIST,
    //     }),
    //     transform: Transform::from_translation(Vec3::new(5.0, 0.0, 5.0)),
    // });

    commands
        .spawn(CameraBundle {
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 10.0))
                .looking_at(Vec3::ZERO, Vec3::Y),
            camera: Camera {
                projection: Mat4::perspective_infinite_reverse_rh(
                    60_f32.to_radians(),
                    16.0 / 9.0,
                    0.1,
                ),
            },
        })
        .insert(CameraController::default());
}

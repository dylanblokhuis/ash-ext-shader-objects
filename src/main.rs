#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use ash::vk::PrimitiveTopology;
use bevy::app::PluginGroupBuilder;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::diagnostic::LogDiagnosticsPlugin;
use bevy::prelude::*;
use bevy::window::close_on_esc;
use bevy::window::WindowMode;
use bevy_runner::config::VulkanSettings;
use bevy_runner::VulkanWinitPlugin;
use game::camera_controller::CameraController;
use game::camera_controller::CameraControllerPlugin;
use render::bundles::Camera;
use render::bundles::CameraBundle;
use render::bundles::MaterialMeshBundle;
use render::mesh::Mesh;
use render::mesh::Vertex;
use render::primitives;
use render::primitives::Box;
use render::RenderPlugin;
use std::default::Default;

mod bevy_runner;
mod buffer;
mod chunky_list;
mod ctx;
mod game;
mod passes;
mod render;

pub struct PluginBundle;

impl PluginGroup for PluginBundle {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<PluginBundle>()
            .add(bevy::input::InputPlugin)
            .add(bevy::window::WindowPlugin::default())
            .add(VulkanWinitPlugin::default())
    }
}

fn main() {
    App::new()
        .insert_resource(VulkanSettings::default())
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
        .add_plugin(VulkanWinitPlugin::default())
        .add_plugin(RenderPlugin::default())
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin)
        .add_plugin(CameraControllerPlugin)
        .add_system(close_on_esc)
        .add_startup_system(spawn_stuff)
        .run();
}

fn spawn_stuff(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    for x in 0..40 {
        for y in 0..40 {
            commands.spawn(MaterialMeshBundle {
                mesh: meshes.add(Box::new(1.0, 1.0, 1.0).into()),
                transform: Transform::from_translation(Vec3::new(
                    x as f32 * 2.0,
                    y as f32 * 2.0,
                    x as f32 * 2.0,
                )),
            });
        }
    }

    commands.spawn(MaterialMeshBundle {
        mesh: meshes.add(Box::new(1.0, 1.0, 1.0).into()),
        transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
    });

    commands.spawn(MaterialMeshBundle {
        mesh: meshes.add(Box::new(1.0, 1.0, 1.0).into()),
        transform: Transform::from_translation(Vec3::new(5.0, 5.0, 5.0)),
    });

    commands.spawn(MaterialMeshBundle {
        mesh: meshes.add(primitives::Cube::new(2.0).into()),
        transform: Transform::from_translation(Vec3::new(1.0, 1.0, 1.0)),
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

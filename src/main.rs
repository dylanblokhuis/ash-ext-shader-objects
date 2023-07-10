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
        .add_plugins(RenderPlugin::default())
        .add_plugins(LogDiagnosticsPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin)
        .add_plugins(CameraControllerPlugin)
        .add_systems(Update, close_on_esc)
        .add_systems(Startup, spawn_stuff)
        .run();
}

fn spawn_stuff(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Material>>,
    asset_server: Res<AssetServer>,
) {
    let _ = info_span!("Spawning objects");
    for x in 0..50 {
        for y in 0..50 {
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

    commands.spawn(MaterialMeshBundle {
        mesh: meshes.add(primitives::Cube::new(2.0).into()),
        transform: Transform::from_translation(Vec3::new(-10.0, 1.0, 1.0)),
        material: materials.add(Material {
            base_color: Vec3::new(1.0, 0.0, 0.0),
            base_color_texture: Some(asset_server.load("images/public.png")),
            ..Default::default()
        }),
    });

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

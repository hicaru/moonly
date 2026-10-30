#[allow(dead_code)]
mod support;

use bevy::{
    app::AppExit, camera::Exposure, pbr::DistanceFog, prelude::*, render::camera::CameraRenderGraph,
};
use moonly::prelude::*;
use support::{
    CaptureOptions, CaptureState, capture_system, deterministic_time, float_arg, native_reference,
    window_plugin,
};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — Volumetric Light Tunnel")))
        .add_plugins((
            MoonlyCorePlugin,
            MoonlyCullPlugin,
            MoonlyShadowsPlugin,
            MoonlyGiPlugin,
            MoonlyRenderPlugin,
        ))
        .insert_resource(MoonlyConfig {
            volumetrics: VolumetricQualityConfig {
                quality: float_arg("--vol-quality", 1.0),
                fade_distance_meters: float_arg("--vol-fade", 80.0),
                ..VolumetricQualityConfig::default()
            },
            ..MoonlyConfig::moonly_r4()
        })
        .insert_resource(WeatherEnvironment {
            fog_density: 0.012,
            fog_height_falloff: 0.04,
            ..WeatherEnvironment::default()
        })
        .insert_resource(Hdr10PostConfig {
            exposure_mode: MoonlyExposureMode::Camera,
            ..Hdr10PostConfig::default()
        })
        .insert_resource(ClearColor(Color::srgb(0.04, 0.055, 0.09)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/volumetric-tunnel.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(deterministic_time())
        .add_systems(
            Startup,
            (native_reference::setup_offscreen_capture, setup_tunnel).chain(),
        )
        .add_systems(Update, (move_through_tunnel, capture_system).chain())
        .run()
}

fn setup_tunnel(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.23, 0.28),
        perceptual_roughness: 0.72,
        ..StandardMaterial::default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(9.0, 30.0)))),
        MeshMaterial3d(stone.clone()),
        Transform::from_xyz(0.0, 0.0, -8.0),
    ));
    spawn_columns(&mut commands, &mut meshes, &mut materials, stone);
    spawn_tunnel_lights(&mut commands);
    commands.spawn((
        MoonlyCamera::default(),
        support::reference_scene::debug_view_from_args(),
        CameraRenderGraph::new(MoonlyRender),
        Exposure { ev100: 0.0 },
        DistanceFog::default(),
        MoonlyWeatherFog,
        Transform::from_xyz(5.5, 3.2, 11.0).looking_at(Vec3::new(0.0, 1.5, -4.0), Vec3::Y),
    ));
}

fn spawn_columns(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    stone: Handle<StandardMaterial>,
) {
    let column = meshes.add(Cylinder::new(0.45, 4.5));
    let cap = meshes.add(Cylinder::new(0.62, 0.18));
    for row in 0..6 {
        let z = 3.0 - row as f32 * 3.2;
        for x in [-3.2, 3.2] {
            commands.spawn((
                Mesh3d(column.clone()),
                MeshMaterial3d(stone.clone()),
                Transform::from_xyz(x, 2.25, z),
            ));
            commands.spawn((
                Mesh3d(cap.clone()),
                MeshMaterial3d(stone.clone()),
                Transform::from_xyz(x, 4.5, z),
            ));
        }
    }
    let inlay = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::new(0.12, 0.32, 0.65, 1.0),
        ..StandardMaterial::default()
    });
    let strip = meshes.add(Cuboid::new(0.05, 0.02, 26.0));
    for x in [-1.6, 1.6] {
        commands.spawn((
            Mesh3d(strip.clone()),
            MeshMaterial3d(inlay.clone()),
            Transform::from_xyz(x, 0.02, -7.0),
        ));
    }
}

fn spawn_tunnel_lights(commands: &mut Commands) {
    let mut warm = MoonlyLight::spot(
        Vec3::new(-1.8, 4.2, 4.0),
        Vec3::new(0.25, -0.3, -1.0).normalize(),
        Vec3::new(8.0, 4.2, 1.8),
        28.0,
        1.05,
    );
    warm.volumetric.intensity = 1.2;
    let mut cool = MoonlyLight::spot(
        Vec3::new(2.2, 3.6, -6.0),
        Vec3::new(-0.25, -0.15, 1.0).normalize(),
        Vec3::new(0.5, 2.4, 5.0),
        24.0,
        0.95,
    );
    cool.volumetric.intensity = 0.9;
    if std::env::args().any(|arg| arg == "--no-volumetric") {
        warm.volumetric.intensity = 0.0;
        cool.volumetric.intensity = 0.0;
    }
    commands.spawn(warm);
    commands.spawn(cool);
}

fn move_through_tunnel(time: Res<Time>, mut cameras: Query<&mut Transform, With<MoonlyCamera>>) {
    let seconds = time.elapsed_secs();
    for mut transform in &mut cameras {
        let travel = if std::env::args().any(|arg| arg == "--vol-path") {
            seconds * 1.8
        } else {
            seconds * 0.15
        };
        transform.translation = Vec3::new(5.5 + (seconds * 0.2).sin() * 0.4, 3.2, 11.0 - travel);
        transform.look_at(Vec3::new(0.0, 1.5, -4.0), Vec3::Y);
    }
}

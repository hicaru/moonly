//! Reproducible frame benchmark for Moonly lighting scenarios.

#[allow(dead_code)]
mod support;

use bevy::{app::AppExit, ecs::system::SystemParam, prelude::*, render::camera::CameraRenderGraph};
use moonly::prelude::*;
use support::{
    benchmark::{
        BenchmarkOptions, BenchmarkState, begin_cpu_frame, end_cpu_frame, scenario_config,
        spawn_scenario_lights,
    },
    capture::{CaptureOptions, CaptureState, capture_system},
    deterministic_time,
    reference::BenchmarkScenario,
    scene::{spawn_camera, spawn_test_scene},
    sized_window_plugin,
};

fn main() -> AppExit {
    let options = BenchmarkOptions::from_env_args();
    let Some(scenario) = BenchmarkScenario::parse(&options.scenario_name) else {
        eprintln!("Unknown benchmark scenario: {}", options.scenario_name);
        return AppExit::error();
    };
    let mut config = scenario_config(options.tier);
    if std::env::args().any(|arg| arg == "--no-ao") {
        config.ao_mode = AmbientOcclusionMode::Off;
    }
    let mut post = Hdr10PostConfig::default();
    if std::env::args().any(|arg| arg == "--no-bloom") {
        post.bloom_intensity = 0.0;
    }
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(sized_window_plugin(
        "Moonly — Render Benchmark",
        options.width,
        options.height,
    )))
    .add_plugins(MoonlyPlugin { config })
    .insert_resource(post)
    .insert_resource(ClearColor(Color::srgb(0.06, 0.08, 0.12)))
    .insert_resource(BenchmarkState::new(options.measured_frames as usize))
    .insert_resource(CaptureOptions::from_env_args(
        "target/visual-audit/render-benchmark.png",
    ))
    .insert_resource(CaptureState::default())
    .insert_resource(options)
    .insert_resource(scenario)
    .insert_resource(deterministic_time())
    .add_systems(Startup, (setup_benchmark, configure_volumetrics).chain())
    .add_systems(Update, capture_system)
    .add_systems(First, begin_cpu_frame)
    .add_systems(Last, end_cpu_frame);
    app.run()
}

fn configure_volumetrics(mut lights: Query<&mut MoonlyLight>) {
    if !std::env::args().any(|arg| arg == "--no-volumetrics") {
        return;
    }
    for mut light in &mut lights {
        light.flags.0 &= !MoonlyLightFlags::VOLUMETRIC.0;
    }
}

#[derive(SystemParam)]
struct SceneAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    moonly_materials: ResMut<'w, Assets<MoonlyLitMaterial>>,
    standard_materials: ResMut<'w, Assets<StandardMaterial>>,
}

fn setup_benchmark(
    mut commands: Commands,
    options: Res<BenchmarkOptions>,
    scenario: Res<BenchmarkScenario>,
    mut assets: SceneAssets,
) {
    if options.renderer == "legacy_forward_analytic" {
        spawn_camera(&mut commands);
        spawn_test_scene(
            &mut commands,
            &mut assets.meshes,
            &mut assets.moonly_materials,
        );
    } else {
        spawn_gbuffer_scene(&mut commands, &mut assets);
    }
    spawn_scenario_lights(&mut commands, *scenario);
}

fn spawn_gbuffer_scene(commands: &mut Commands, assets: &mut SceneAssets) {
    let ground = assets.standard_materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.28, 0.25),
        perceptual_roughness: 0.85,
        ..Default::default()
    });
    commands.spawn((
        Mesh3d(assets.meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(6.0)))),
        MeshMaterial3d(ground),
        MoonlyHomOccluder::default(),
    ));
    let stone = assets.standard_materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.31, 0.16),
        perceptual_roughness: 0.65,
        ..Default::default()
    });
    commands.spawn((
        Mesh3d(assets.meshes.add(Cuboid::new(2.0, 2.0, 2.0))),
        MeshMaterial3d(stone),
        Transform::from_xyz(0.0, 1.0, 0.0),
        MoonlyHomOccluder::default(),
    ));
    let metal = assets.standard_materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.35, 0.48),
        metallic: 0.7,
        perceptual_roughness: 0.3,
        ..Default::default()
    });
    commands.spawn((
        Mesh3d(assets.meshes.add(Sphere::new(0.8))),
        MeshMaterial3d(metal),
        Transform::from_xyz(2.5, 0.8, -1.0),
        MoonlyHomOccluder::default(),
    ));
    let emissive = assets.standard_materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::new(0.8, 0.25, 0.05, 1.0),
        ..Default::default()
    });
    commands.spawn((
        Mesh3d(assets.meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
        MeshMaterial3d(emissive),
        Transform::from_xyz(-2.0, 0.25, 0.5),
        MoonlyHomOccluder::default(),
    ));
    commands.spawn((
        MoonlyCamera::default(),
        CameraRenderGraph::new(MoonlyRender),
        Transform::from_xyz(5.5, 4.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
}

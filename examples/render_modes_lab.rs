#[allow(dead_code)]
mod support;

use bevy::{app::AppExit, prelude::*};
use moonly::prelude::*;
use support::{
    CaptureOptions, CaptureState, capture_system, deterministic_time, float_arg, numeric_arg,
    scene::{MotionEnabled, animate_scene, spawn_camera, spawn_test_scene},
    window_plugin,
};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — Render Mode Matrix")))
        .add_plugins(MoonlyPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.08, 0.10, 0.14)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/render-modes.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(MotionEnabled::from_env_args())
        .insert_resource(deterministic_time())
        .add_systems(Startup, configure_scene)
        .add_systems(Update, (animate_scene, capture_system).chain())
        .run()
}

fn configure_scene(
    mut commands: Commands,
    mut config: ResMut<MoonlyConfig>,
    mut shafts: ResMut<SunShaftsConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
) {
    config.tier = match numeric_arg("--tier", 3) {
        0 => MoonlyPipelineTier::ClassicR2,
        1 => MoonlyPipelineTier::MonolithR3,
        2 => MoonlyPipelineTier::MonolithR4,
        _ => MoonlyPipelineTier::MoonlyEnhanced,
    };
    config.shadow_filter = match numeric_arg("--shadow", 1) {
        0 => ShadowFilterMode::HardwarePcf,
        1 => ShadowFilterMode::MonolithJitter64,
        2 => ShadowFilterMode::MonolithMinMax,
        _ => ShadowFilterMode::EnhancedPcss,
    };
    config.ao_mode = match numeric_arg("--ao", 3) {
        0 => AmbientOcclusionMode::Off,
        1 => AmbientOcclusionMode::Ssao,
        2 => AmbientOcclusionMode::Hdao,
        3 => AmbientOcclusionMode::ActivisionColoredAo,
        _ => AmbientOcclusionMode::EnhancedSsdo,
    };
    config.rain_wetness = float_arg("--wetness", 0.0).clamp(0.0, 1.0);
    shafts.mode = match numeric_arg("--shafts", 3) {
        0 => SunShaftsMode::Off,
        1 => SunShaftsMode::Volumetric,
        2 => SunShaftsMode::ScreenSpace,
        _ => SunShaftsMode::Combined,
    };
    shafts.density = float_arg("--density", shafts.density);
    shafts.mie_anisotropy = float_arg("--anisotropy", shafts.mie_anisotropy);
    info!(
        "Render preset: {:?}, {:?}, {:?}, {:?}, wetness {}",
        config.tier, config.shadow_filter, config.ao_mode, shafts.mode, config.rain_wetness
    );
    spawn_camera(&mut commands);
    spawn_test_scene(&mut commands, &mut meshes, &mut materials);
}

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
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — HDR and Optics Matrix")))
        .add_plugins(MoonlyPlugin::enhanced())
        .insert_resource(ClearColor(Color::srgb(0.025, 0.03, 0.05)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/post-optics.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(MotionEnabled::from_env_args())
        .insert_resource(deterministic_time())
        .add_systems(Startup, configure_post_scene)
        .add_systems(Update, (animate_scene, capture_system).chain())
        .run()
}

fn configure_post_scene(
    mut commands: Commands,
    mut post: ResMut<Hdr10PostConfig>,
    mut optics: ResMut<OpticsMode>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
) {
    post.operator = match numeric_arg("--tonemap", 4) {
        0 => TonemapOperator::ClassicMoonly,
        1 => TonemapOperator::ExtendedReinhardW2,
        2 => TonemapOperator::ExtendedReinhardW3,
        3 => TonemapOperator::AcesNarkowicz,
        4 => TonemapOperator::AcesHill,
        5 => TonemapOperator::Uchimura,
        6 => TonemapOperator::SteveM,
        7 => TonemapOperator::Uncharted2,
        _ => TonemapOperator::AgX,
    };
    *optics = match numeric_arg("--optics", 0) {
        1 => OpticsMode::NightVisionGreen,
        2 => OpticsMode::NightVisionWhite,
        3 => OpticsMode::ThermalHeatVision,
        4 => OpticsMode::ParallaxScope,
        _ => OpticsMode::Standard,
    };
    post.exposure = float_arg("--exposure", post.exposure);
    post.contrast = float_arg("--contrast", post.contrast);
    post.vibrance = float_arg("--vibrance", post.vibrance);
    post.bloom_intensity = float_arg("--bloom", post.bloom_intensity);
    post.flare_intensity = float_arg("--flare", post.flare_intensity);
    post.max_nits = float_arg("--nits", post.max_nits);
    info!("Post preset: {:?}, {:?}", post, optics);
    spawn_camera(&mut commands);
    spawn_test_scene(&mut commands, &mut meshes, &mut materials);
    spawn_hdr_chart(&mut commands, &mut meshes, &mut materials);
}

fn spawn_hdr_chart(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
) {
    let mesh = meshes.add(Cuboid::new(0.65, 0.65, 0.65));
    for (index, color) in [
        Vec3::new(8.0, 0.05, 0.02),
        Vec3::new(0.02, 8.0, 0.05),
        Vec3::new(0.02, 0.1, 8.0),
        Vec3::splat(12.0),
    ]
    .into_iter()
    .enumerate()
    {
        let mut material = MoonlyLitMaterial::from_albedo_roughness(Vec3::splat(0.05), 0.3);
        material.uniform.emissive_ao = color.extend(1.0);
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(materials.add(material)),
            Transform::from_xyz(-1.5 + index as f32, 2.2, -1.8),
        ));
    }
}

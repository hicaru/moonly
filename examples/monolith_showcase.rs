//! Visual showcase of analytic blockers, CPU GI, volumetric approximation, and HDR operators.

#[allow(dead_code)]
mod support;

use bevy::{app::AppExit, prelude::*};
use moonly::prelude::*;
use support::{CaptureOptions, CaptureState, capture_system, deterministic_time, window_plugin};

#[derive(Resource, Debug, Clone, Copy, Default)]
struct ShowcasePreset(u32);

impl ShowcasePreset {
    fn from_env_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let index = args.iter().position(|arg| arg == "--preset");
        Self(
            index
                .and_then(|i| args.get(i + 1))
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
        )
    }
}

/// Marker component for the rotating central Monolith crystal.
#[derive(Component)]
struct MonolithCrystal;

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin(
            "Moonly — Moonly-Monolith R4 Lighting & GI Showcase",
        )))
        .add_plugins(MoonlyPlugin::enhanced())
        .insert_resource(ClearColor(Color::srgb(0.18, 0.24, 0.34)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/showcase.png",
        ))
        .insert_resource(ShowcasePreset::from_env_args())
        .insert_resource(CaptureState::default())
        .insert_resource(deterministic_time())
        .add_systems(Startup, setup_showcase_scene)
        .add_systems(
            Update,
            (animate_showcase_scene, handle_optics_keys, capture_system).chain(),
        )
        .run()
}

fn setup_showcase_scene(
    mut commands: Commands,
    preset: Res<ShowcasePreset>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
    mut config: ResMut<MoonlyConfig>,
) {
    if preset.0 == 1 {
        config.rain_wetness = 0.85;
    }

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.8, 7.6).looking_at(Vec3::new(0.0, 1.1, 0.0), Vec3::Y),
    ));

    spawn_architecture(&mut commands, &mut meshes, &mut materials);
    spawn_lights_and_gi(&mut commands, &mut meshes, &mut materials);
}

fn spawn_architecture(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
) {
    let floor_mesh = meshes.add(Cuboid::new(16.0, 0.4, 16.0));
    let floor_mat = materials.add(MoonlyLitMaterial::from_albedo_roughness(
        Vec3::new(0.52, 0.50, 0.47),
        0.38,
    ));
    commands.spawn((
        Mesh3d(floor_mesh),
        MeshMaterial3d(floor_mat),
        Transform::from_xyz(0.0, -0.2, 0.0),
        GiReflectorBox::from_center_extents(
            Vec3::new(0.0, -0.2, 0.0),
            Vec3::new(8.0, 0.2, 8.0),
            Vec3::new(0.65, 0.58, 0.50),
        ),
    ));

    let pillar_mesh = meshes.add(Cylinder::new(0.75, 3.6));
    let left_mat = materials.add(MoonlyLitMaterial::from_albedo_roughness(
        Vec3::new(0.82, 0.32, 0.24),
        0.45,
    ));
    let right_mat = materials.add(MoonlyLitMaterial::from_albedo_roughness(
        Vec3::new(0.24, 0.52, 0.84),
        0.45,
    ));
    commands.spawn((
        Mesh3d(pillar_mesh.clone()),
        MeshMaterial3d(left_mat),
        Transform::from_xyz(-2.2, 1.8, 0.0),
        GiReflectorBox::from_center_extents(
            Vec3::new(-2.2, 1.8, 0.0),
            Vec3::new(0.8, 1.8, 0.8),
            Vec3::new(0.88, 0.30, 0.22),
        ),
    ));
    commands.spawn((
        Mesh3d(pillar_mesh),
        MeshMaterial3d(right_mat),
        Transform::from_xyz(2.2, 1.8, 0.0),
        GiReflectorBox::from_center_extents(
            Vec3::new(2.2, 1.8, 0.0),
            Vec3::new(0.8, 1.8, 0.8),
            Vec3::new(0.22, 0.55, 0.88),
        ),
    ));

    let arch_mesh = meshes.add(Cuboid::new(6.4, 0.9, 1.1));
    let arch_mat = materials.add(MoonlyLitMaterial::from_albedo_roughness(
        Vec3::new(0.68, 0.66, 0.62),
        0.50,
    ));
    commands.spawn((
        Mesh3d(arch_mesh),
        MeshMaterial3d(arch_mat),
        Transform::from_xyz(0.0, 3.2, -2.2),
        GiReflectorBox::from_center_extents(
            Vec3::new(0.0, 3.2, -2.2),
            Vec3::new(3.2, 0.45, 0.55),
            Vec3::new(0.70, 0.68, 0.64),
        ),
    ));
}

fn spawn_lights_and_gi(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
) {
    let mut crystal_mat =
        MoonlyLitMaterial::from_albedo_roughness(Vec3::new(0.86, 0.88, 0.94), 0.18);
    crystal_mat.uniform.emissive_ao = Vec4::new(0.08, 0.16, 0.28, 1.0);
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.85).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(crystal_mat)),
        Transform::from_xyz(0.0, 1.15, 0.0),
        MonolithCrystal,
        HemisphereTracker::default(),
    ));

    let ped_mesh = meshes.add(Cylinder::new(1.1, 0.35));
    let ped_mat = materials.add(MoonlyLitMaterial::from_albedo_roughness(
        Vec3::new(0.38, 0.39, 0.42),
        0.30,
    ));
    commands.spawn((
        Mesh3d(ped_mesh),
        MeshMaterial3d(ped_mat),
        Transform::from_xyz(0.0, 0.175, 0.0),
    ));

    commands.spawn((
        MoonlyLight::spot(
            Vec3::new(0.0, 4.6, 1.2),
            Vec3::new(0.0, -0.95, -0.25).normalize(),
            Vec3::new(2.8, 2.1, 1.3),
            16.0,
            1.25,
        ),
        Transform::from_xyz(0.0, 4.6, 1.2),
        GlobalTransform::default(),
    ));
}

fn animate_showcase_scene(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<MonolithCrystal>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
) {
    let seconds = time.elapsed_secs();
    let position = Vec3::new((seconds * 0.8).sin(), 1.15 + seconds.cos() * 0.25, 0.0);
    for mut transform in &mut query {
        transform.translation = position;
        transform.rotate_y(time.delta_secs() * 0.6);
    }
    for (_, material) in materials.iter_mut() {
        material.uniform.blocker_0 = position.extend(0.85);
    }
}

fn handle_optics_keys(keys: Res<ButtonInput<KeyCode>>, mut optics: ResMut<OpticsMode>) {
    if keys.just_pressed(KeyCode::KeyN) {
        *optics = match *optics {
            OpticsMode::Standard => OpticsMode::NightVisionGreen,
            OpticsMode::NightVisionGreen => OpticsMode::NightVisionWhite,
            OpticsMode::NightVisionWhite => OpticsMode::ThermalHeatVision,
            OpticsMode::ThermalHeatVision => OpticsMode::ParallaxScope,
            OpticsMode::ParallaxScope => OpticsMode::Standard,
        };
    }
}

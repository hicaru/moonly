//! Interactive and automated GI radiosity, wet rain shadow, and optics laboratory example.

#[allow(dead_code)]
mod support;

use bevy::{app::AppExit, prelude::*};
use moonly::prelude::*;
use support::{
    CaptureOptions, CaptureState, capture_system, deterministic_time,
    scene::{ExampleLight, ExampleOrb, MotionEnabled, animate_scene, spawn_camera},
    window_plugin,
};

#[derive(Resource, Debug, Clone, Copy, Default)]
struct LabOptions {
    optics_index: u32,
}

impl LabOptions {
    fn from_env_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let index = args.iter().position(|arg| arg == "--optics");
        Self {
            optics_index: index
                .and_then(|i| args.get(i + 1))
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
        }
    }
}

fn main() -> AppExit {
    let config = MoonlyConfig {
        rain_wetness: 0.65,
        ..MoonlyConfig::default()
    };
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin(
            "Moonly — GI Radiosity, Rain Wetness & Optics Lab",
        )))
        .add_plugins(MoonlyPlugin { config })
        .insert_resource(ClearColor(Color::srgb(0.14, 0.18, 0.25)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/gi-lab.png",
        ))
        .insert_resource(LabOptions::from_env_args())
        .insert_resource(CaptureState::default())
        .insert_resource(MotionEnabled::from_env_args())
        .insert_resource(deterministic_time())
        .add_systems(Startup, setup_gi_lab_scene)
        .add_systems(Update, (animate_scene, capture_system).chain())
        .run()
}

const LAB_BLOCKERS: [Vec4; 4] = [
    Vec4::new(-1.6, 0.75, 0.2, 0.75),
    Vec4::new(0.0, 0.75, 0.2, 0.75),
    Vec4::new(1.6, 0.75, 0.2, 0.75),
    Vec4::ZERO,
];

fn setup_gi_lab_scene(
    mut commands: Commands,
    options: Res<LabOptions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
    mut post_cfg: ResMut<Hdr10PostConfig>,
) {
    post_cfg.operator = TonemapOperator::AgX;
    post_cfg.exposure = 1.25;

    spawn_camera(&mut commands);

    spawn_cornell_bunker(&mut commands, &mut meshes, &mut materials);
    spawn_bunker_spheres_and_lights(
        &mut commands,
        &mut meshes,
        &mut materials,
        options.optics_index,
    );
}

fn spawn_cornell_bunker(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
) {
    let floor_mesh = meshes.add(Cuboid::new(14.0, 0.3, 14.0));
    let floor_mat = materials.add(
        MoonlyLitMaterial::from_albedo_roughness(Vec3::new(0.46, 0.45, 0.43), 0.16)
            .with_blockers(LAB_BLOCKERS),
    );
    commands.spawn((
        Mesh3d(floor_mesh),
        MeshMaterial3d(floor_mat),
        Transform::from_xyz(0.0, -0.15, 0.0),
        GiReflectorBox::from_center_extents(
            Vec3::new(0.0, -0.15, 0.0),
            Vec3::new(7.0, 0.15, 7.0),
            Vec3::new(0.55, 0.54, 0.52),
        ),
    ));

    let wall_mesh = meshes.add(Cuboid::new(0.4, 4.0, 6.0));
    let warm_wall = materials.add(
        MoonlyLitMaterial::from_albedo_roughness(Vec3::new(0.86, 0.36, 0.18), 0.55)
            .with_blockers(LAB_BLOCKERS),
    );
    let cool_wall = materials.add(
        MoonlyLitMaterial::from_albedo_roughness(Vec3::new(0.18, 0.68, 0.78), 0.55)
            .with_blockers(LAB_BLOCKERS),
    );
    commands.spawn((
        Mesh3d(wall_mesh.clone()),
        MeshMaterial3d(warm_wall),
        Transform::from_xyz(-3.4, 2.0, -0.5),
        GiReflectorBox::from_center_extents(
            Vec3::new(-3.4, 2.0, -0.5),
            Vec3::new(0.2, 2.0, 3.0),
            Vec3::new(0.90, 0.38, 0.18),
        ),
    ));
    commands.spawn((
        Mesh3d(wall_mesh),
        MeshMaterial3d(cool_wall),
        Transform::from_xyz(3.4, 2.0, -0.5),
        GiReflectorBox::from_center_extents(
            Vec3::new(3.4, 2.0, -0.5),
            Vec3::new(0.2, 2.0, 3.0),
            Vec3::new(0.18, 0.72, 0.85),
        ),
    ));
}

fn spawn_bunker_spheres_and_lights(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
    optics_index: u32,
) {
    let sphere_mesh = meshes.add(Sphere::new(0.75).mesh().uv(32, 18));
    for (idx, x_pos) in [-1.6_f32, 0.0, 1.6].into_iter().enumerate() {
        let roughness = 0.12 + (idx as f32) * 0.32;
        let mat = materials.add(
            MoonlyLitMaterial::from_albedo_roughness(Vec3::new(0.82, 0.80, 0.76), roughness)
                .with_blockers(LAB_BLOCKERS),
        );
        commands.spawn((
            Mesh3d(sphere_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_xyz(x_pos, 0.75, 0.2),
            HemisphereTracker::default(),
            ExampleOrb(idx as u32),
        ));
    }

    commands.spawn((
        XRayLight::spot(
            Vec3::new(-1.2, 4.2, 1.5),
            Vec3::new(0.25, -0.92, -0.28).normalize(),
            Vec3::new(3.4, 2.5, 1.6),
            18.0,
            1.35,
        ),
        Transform::from_xyz(-1.2, 4.2, 1.5),
        GlobalTransform::default(),
        ExampleLight,
    ));
    commands.spawn((
        XRayLight {
            flags: XRayLightFlags::GI_ENABLED,
            color: Vec3::new(0.5, 0.8, 1.4),
            position: Vec3::new(1.8, 2.4, 1.0),
            range: 10.0,
            ..XRayLight::default()
        },
        Transform::from_xyz(1.8, 2.4, 1.0),
        GlobalTransform::default(),
    ));
    commands.spawn((
        XRayLight {
            light_type: XRayLightType::Direct,
            flags: XRayLightFlags::GI_ENABLED,
            color: Vec3::new(0.18, 0.16, 0.13),
            direction: Vec3::new(-0.4, -0.8, -0.3).normalize(),
            range: 60.0,
            ..XRayLight::default()
        },
        Transform::default().looking_at(Vec3::new(-0.4, -0.8, -0.3), Vec3::Y),
    ));

    let mode = match optics_index {
        1 => OpticsMode::NightVisionGreen,
        2 => OpticsMode::NightVisionWhite,
        3 => OpticsMode::ThermalHeatVision,
        4 => OpticsMode::ParallaxScope,
        _ => OpticsMode::Standard,
    };
    commands.insert_resource(mode);
}

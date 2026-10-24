//! Visual showcase of XRay-Monolith R4 dynamic shadows, 32-photon GI, volumetrics, and HDR10 in Bevy.

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::WindowResolution,
};
use moonly::prelude::*;

/// CLI capture options for automated visual artifact verification.
#[derive(Resource, Debug, Clone, Default)]
struct CaptureOptions {
    screenshot_frame: Option<u32>,
    screenshot_path: String,
    exit_frame: Option<u32>,
    preset_mode: u32,
}

impl CaptureOptions {
    fn from_env_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut opts = Self {
            screenshot_path: "target/moonly_showcase.png".to_string(),
            ..Self::default()
        };
        let mut i = 1_usize;
        while let Some(arg) = args.get(i) {
            if arg == "--screenshot-after" {
                if let Some(frame_str) = args.get(i + 1) {
                    opts.screenshot_frame = frame_str.parse::<u32>().ok();
                }
                if let Some(path_str) = args.get(i + 2) {
                    opts.screenshot_path.clone_from(path_str);
                }
                i += 3;
            } else if arg == "--exit-after" {
                if let Some(frame_str) = args.get(i + 1) {
                    opts.exit_frame = frame_str.parse::<u32>().ok();
                }
                i += 2;
            } else if arg == "--preset" {
                if let Some(preset_str) = args.get(i + 1) {
                    opts.preset_mode = preset_str.parse::<u32>().unwrap_or(0);
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        opts
    }
}

/// Marker component for the rotating central Monolith crystal.
#[derive(Component)]
struct MonolithCrystal;

fn main() {
    let capture = CaptureOptions::from_env_args();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Moonly — XRay-Monolith R4 Lighting & GI Showcase".to_string(),
                resolution: WindowResolution::new(1280, 720),
                ..Window::default()
            }),
            ..WindowPlugin::default()
        }))
        .add_plugins(MoonlyPlugin::enhanced())
        .insert_resource(ClearColor(Color::srgb(0.18, 0.24, 0.34)))
        .insert_resource(capture)
        .add_systems(Startup, setup_showcase_scene)
        .add_systems(Update, (animate_showcase_scene, handle_capture_and_keys))
        .run();
}

fn setup_showcase_scene(
    mut commands: Commands,
    capture: Res<CaptureOptions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
    mut config: ResMut<MoonlyConfig>,
) {
    if capture.preset_mode == 1 {
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
        XRayLight::spot(
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
) {
    for mut xf in &mut query {
        xf.rotate_y(time.delta_secs() * 0.6);
    }
}

fn handle_capture_and_keys(
    mut commands: Commands,
    inputs: (Res<CaptureOptions>, Res<ButtonInput<KeyCode>>),
    mut optics: ResMut<OpticsMode>,
    mut exit: MessageWriter<AppExit>,
    mut frame: Local<u32>,
) {
    let (capture, keys) = inputs;
    *frame += 1;
    if keys.just_pressed(KeyCode::KeyN) {
        *optics = match *optics {
            OpticsMode::Standard => OpticsMode::NightVisionGreen,
            OpticsMode::NightVisionGreen => OpticsMode::NightVisionWhite,
            OpticsMode::NightVisionWhite => OpticsMode::ThermalHeatVision,
            OpticsMode::ThermalHeatVision | OpticsMode::ParallaxScope => OpticsMode::Standard,
        };
    }
    if capture.screenshot_frame == Some(*frame) {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(capture.screenshot_path.clone()));
    }
    if capture.exit_frame == Some(*frame) {
        exit.write(AppExit::Success);
    }
}

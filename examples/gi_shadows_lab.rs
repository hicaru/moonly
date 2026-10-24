//! Interactive and automated GI radiosity, wet rain shadow, and optics laboratory example.

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
    window::WindowResolution,
};
use moonly::prelude::*;

/// CLI options for automated screenshot capture and optic/tonemap preset selection.
#[derive(Resource, Debug, Clone, Default)]
struct LabCaptureOptions {
    screenshot_frame: Option<u32>,
    screenshot_path: String,
    exit_frame: Option<u32>,
    optics_index: u32,
}

impl LabCaptureOptions {
    fn from_env_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut opts = Self {
            screenshot_path: "target/moonly_gi_lab.png".to_string(),
            ..Self::default()
        };
        let mut i = 1_usize;
        while let Some(arg) = args.get(i) {
            if arg == "--screenshot-after" {
                if let Some(f) = args.get(i + 1) {
                    opts.screenshot_frame = f.parse::<u32>().ok();
                }
                if let Some(p) = args.get(i + 2) {
                    opts.screenshot_path.clone_from(p);
                }
                i += 3;
            } else if arg == "--exit-after" {
                if let Some(f) = args.get(i + 1) {
                    opts.exit_frame = f.parse::<u32>().ok();
                }
                i += 2;
            } else if arg == "--optics" {
                if let Some(o) = args.get(i + 1) {
                    opts.optics_index = o.parse::<u32>().unwrap_or(0);
                }
                i += 2;
            } else {
                i += 1;
            }
        }
        opts
    }
}

fn main() {
    let capture = LabCaptureOptions::from_env_args();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Moonly — GI Radiosity, Rain Wetness & Optics Lab".to_string(),
                resolution: WindowResolution::new(1280, 720),
                ..Window::default()
            }),
            ..WindowPlugin::default()
        }))
        .add_plugins(MoonlyPlugin::enhanced())
        .insert_resource(ClearColor(Color::srgb(0.14, 0.18, 0.25)))
        .insert_resource(capture)
        .add_systems(Startup, setup_gi_lab_scene)
        .add_systems(Update, tick_gi_lab_capture)
        .run();
}

const LAB_BLOCKERS: [Vec4; 4] = [
    Vec4::new(-1.6, 0.75, 0.2, 0.75),
    Vec4::new(0.0, 0.75, 0.2, 0.75),
    Vec4::new(1.6, 0.75, 0.2, 0.75),
    Vec4::ZERO,
];

fn setup_gi_lab_scene(
    mut commands: Commands,
    capture: Res<LabCaptureOptions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
    mut config: ResMut<MoonlyConfig>,
    mut post_cfg: ResMut<Hdr10PostConfig>,
) {
    config.rain_wetness = 0.65;
    post_cfg.operator = TonemapOperator::AgX;
    post_cfg.exposure = 1.25;

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.4, 6.8).looking_at(Vec3::new(0.0, 1.2, 0.0), Vec3::Y),
    ));

    spawn_cornell_bunker(&mut commands, &mut meshes, &mut materials);
    spawn_bunker_spheres_and_lights(
        &mut commands,
        &mut meshes,
        &mut materials,
        capture.optics_index,
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
    ));

    let mode = match optics_index {
        1 => OpticsMode::NightVisionGreen,
        2 => OpticsMode::NightVisionWhite,
        3 => OpticsMode::ThermalHeatVision,
        _ => OpticsMode::Standard,
    };
    commands.insert_resource(mode);
}

fn tick_gi_lab_capture(
    mut commands: Commands,
    capture: Res<LabCaptureOptions>,
    mut exit: MessageWriter<AppExit>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    if capture.screenshot_frame == Some(*frame) {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(capture.screenshot_path.clone()));
    }
    if capture.exit_frame == Some(*frame) {
        exit.write(AppExit::Success);
    }
}

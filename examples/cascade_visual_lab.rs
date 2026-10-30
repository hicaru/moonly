//! Deterministic GPU sun-cascade captures in a procedural dusk landscape.

#[allow(dead_code)]
mod support;

use bevy::{
    app::AppExit,
    camera::{Exposure, RenderTarget},
    pbr::DistanceFog,
    prelude::*,
    render::{camera::CameraRenderGraph, render_resource::TextureFormat},
};
use moonly_core::{
    MoonlyConfig, MoonlyCorePlugin, MoonlyLight, MoonlyLightFlags, MoonlyLightType,
    SunCascadeConfig, WeatherEnvironment,
};
use moonly_cull::MoonlyCullPlugin;
use moonly_gi::MoonlyGiPlugin;
use moonly_post::{Hdr10PostConfig, MoonlyExposureMode};
use moonly_render::{MoonlyCamera, MoonlyRender, MoonlyRenderPlugin, MoonlyWeatherFog};
use moonly_shadows::MoonlyShadowsPlugin;
use support::{
    CAPTURE_HEIGHT, CAPTURE_WIDTH, CaptureOptions, CaptureState, capture::OffscreenCapture,
    capture_system, deterministic_time, window_plugin,
};

const CAMERA_ORIGIN: Vec3 = Vec3::new(0.0, 9.0, 20.0);
const CAMERA_TARGET_OFFSET: Vec3 = Vec3::new(0.0, -8.0, -60.0);
const CASCADE_SPEED_METERS_PER_FRAME: f32 = 0.55;
const SUBTEXEL_STEP_METERS: f32 = 0.000_25;

#[derive(Component)]
struct CascadeLabCamera;

#[derive(Resource, Debug, Clone, Copy, Default)]
struct LabOptions {
    cascade_motion: bool,
    subtexel_motion: bool,
    no_cascade_blend: bool,
}

impl LabOptions {
    fn from_env_args() -> Self {
        Self {
            cascade_motion: std::env::args().any(|arg| arg == "--cascade-motion"),
            subtexel_motion: std::env::args().any(|arg| arg == "--subtexel-motion"),
            no_cascade_blend: std::env::args().any(|arg| arg == "--no-cascade-blend"),
        }
    }
}

fn main() -> AppExit {
    let options = LabOptions::from_env_args();
    let mut sun_cascades = SunCascadeConfig::reference();
    if options.no_cascade_blend {
        sun_cascades.blend_band_fraction = 0.0;
    }
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — Cascade Dusk")))
        .add_plugins((
            MoonlyCorePlugin,
            MoonlyCullPlugin,
            MoonlyShadowsPlugin,
            MoonlyGiPlugin,
            MoonlyRenderPlugin,
        ))
        .insert_resource(MoonlyConfig {
            sun_cascades,
            ..MoonlyConfig::moonly_r4()
        })
        .insert_resource(WeatherEnvironment {
            hemi_color: Vec4::new(0.36, 0.28, 0.52, 0.8),
            ambient_color: Vec3::new(0.1, 0.065, 0.14),
            fog_color: Vec3::new(0.22, 0.12, 0.25),
            fog_near: 30.0,
            fog_far: 420.0,
            fog_density: 0.0035,
            ..WeatherEnvironment::default()
        })
        .insert_resource(Hdr10PostConfig {
            exposure_mode: MoonlyExposureMode::Camera,
            bloom_intensity: 0.16,
            flare_intensity: 0.025,
            ..Hdr10PostConfig::default()
        })
        .insert_resource(ClearColor(Color::srgb(0.035, 0.04, 0.09)))
        .insert_resource(options)
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/cascade-visual.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(deterministic_time())
        .add_systems(Startup, setup_cascade_scene)
        .add_systems(Update, (move_camera, capture_system).chain())
        .run()
}

fn setup_cascade_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut capture: ResMut<CaptureOptions>,
) {
    spawn_landscape(&mut commands, &mut meshes, &mut materials);
    spawn_casters(&mut commands, &mut meshes, &mut materials);
    spawn_horizon(&mut commands, &mut meshes, &mut materials);
    spawn_sun(&mut commands);
    spawn_camera(&mut commands, &mut images, &mut capture);
}

fn surface(
    materials: &mut Assets<StandardMaterial>,
    color: Color,
    glow: f32,
    unlit: bool,
) -> Handle<StandardMaterial> {
    let linear = color.to_linear();
    materials.add(StandardMaterial {
        base_color: color,
        emissive: LinearRgba::rgb(linear.red * glow, linear.green * glow, linear.blue * glow),
        perceptual_roughness: 0.68,
        unlit,
        ..StandardMaterial::default()
    })
}

fn spawn_landscape(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let ground = surface(materials, Color::srgb(0.105, 0.12, 0.17), 0.0, false);
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(70.0, 150.0)))),
        MeshMaterial3d(ground),
        Transform::from_xyz(0.0, 0.0, -80.0),
    ));
    let runway = surface(materials, Color::srgb(0.14, 0.16, 0.22), 0.0, false);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(20.0, 0.035, 290.0))),
        MeshMaterial3d(runway),
        Transform::from_xyz(0.0, 0.018, -80.0),
    ));
    let edge = surface(materials, Color::srgb(0.2, 0.62, 0.67), 0.4, true);
    let edge_mesh = meshes.add(Cuboid::new(0.09, 0.025, 285.0));
    for x in [-10.0, 10.0] {
        commands.spawn((
            Mesh3d(edge_mesh.clone()),
            MeshMaterial3d(edge.clone()),
            Transform::from_xyz(x, 0.045, -80.0),
        ));
    }
    let dash = surface(materials, Color::srgb(0.66, 0.48, 0.63), 0.25, true);
    let dash_mesh = meshes.add(Cuboid::new(0.13, 0.025, 3.5));
    for index in 0..20 {
        commands.spawn((
            Mesh3d(dash_mesh.clone()),
            MeshMaterial3d(dash.clone()),
            Transform::from_xyz(0.0, 0.05, 15.0 - index as f32 * 13.0),
        ));
    }
}

fn spawn_casters(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let casters = [
        (20.0, 4.0, Color::srgb(0.92, 0.24, 0.12)),
        (40.0, 7.0, Color::srgb(0.08, 0.55, 0.78)),
        (160.0, 20.0, Color::srgb(0.9, 0.64, 0.2)),
    ];
    for (index, (distance, height, color)) in casters.into_iter().enumerate() {
        let x = (index as f32 - 1.0) * 7.0;
        let z = CAMERA_ORIGIN.z - distance;
        let radius = height * 0.27;
        let material = surface(materials, color, 0.025, false);
        let rotation = Quat::from_rotation_y(0.3 + index as f32 * 0.35);
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(radius, height * 0.82).mesh().resolution(6))),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(x, height * 0.41, z).with_rotation(rotation),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cone::new(radius, height * 0.18).mesh().resolution(6))),
            MeshMaterial3d(material),
            Transform::from_xyz(x, height * 0.91, z).with_rotation(rotation),
        ));
        let marker = surface(materials, color, 0.32, true);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(34.0, 0.02, 0.12))),
            MeshMaterial3d(marker),
            Transform::from_xyz(0.0, 0.065, z),
        ));
    }
}

fn spawn_horizon(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    for band in 0..12 {
        let t = band as f32 / 11.0;
        let color = Color::srgb(0.46 - t * 0.425, 0.18 - t * 0.125, 0.25 + t * 0.1);
        let material = surface(materials, color, 0.35, true);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(440.0, 18.2, 1.0))),
            MeshMaterial3d(material),
            Transform::from_xyz(0.0, -5.0 + band as f32 * 18.0, -235.0),
        ));
    }
    let mountain = surface(materials, Color::srgb(0.075, 0.055, 0.13), 0.0, false);
    for index in 0..9 {
        let height = 18.0 + (index * 7 % 5) as f32 * 6.0;
        let x = (index as f32 - 4.0) * 32.0;
        commands.spawn((
            Mesh3d(meshes.add(Cone::new(27.0, height).mesh().resolution(5))),
            MeshMaterial3d(mountain.clone()),
            Transform::from_xyz(x, height * 0.5, -220.0 + (index % 3) as f32 * 4.0),
        ));
    }
}

fn spawn_sun(commands: &mut Commands) {
    let direction = Vec3::new(0.72, -0.58, -0.32).normalize();
    commands.spawn((
        MoonlyLight {
            light_type: MoonlyLightType::Direct,
            flags: MoonlyLightFlags::SHADOW,
            color: Vec3::new(3.4, 1.9, 1.25),
            direction,
            range: 250.0,
            ..MoonlyLight::default()
        },
        Transform::from_xyz(0.0, 20.0, 0.0)
            .looking_at(Vec3::new(0.0, 20.0, 0.0) + direction, Vec3::Y),
    ));
}

fn spawn_camera(commands: &mut Commands, images: &mut Assets<Image>, capture: &mut CaptureOptions) {
    spawn_lab_camera(commands, RenderTarget::default());
    if !std::env::args().any(|arg| arg == "--offscreen") {
        return;
    }
    let image = images.add(Image::new_target_texture(
        CAPTURE_WIDTH,
        CAPTURE_HEIGHT,
        TextureFormat::Rgba16Float,
        None,
    ));
    capture.offscreen = Some(OffscreenCapture(image.clone()));
    spawn_lab_camera(commands, RenderTarget::Image(image.into()));
}

fn spawn_lab_camera(commands: &mut Commands, target: RenderTarget) {
    commands.spawn((
        MoonlyCamera::default(),
        Camera::default(),
        target,
        Exposure { ev100: 0.0 },
        DistanceFog::default(),
        MoonlyWeatherFog,
        CameraRenderGraph::new(MoonlyRender),
        CascadeLabCamera,
        Transform::from_translation(CAMERA_ORIGIN)
            .looking_at(CAMERA_ORIGIN + CAMERA_TARGET_OFFSET, Vec3::Y),
    ));
}

fn move_camera(
    options: Res<LabOptions>,
    mut cameras: Query<&mut Transform, With<CascadeLabCamera>>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    let idle = !options.cascade_motion && !options.subtexel_motion;
    for mut transform in &mut cameras {
        if options.cascade_motion {
            transform.translation.z =
                CAMERA_ORIGIN.z - *frame as f32 * CASCADE_SPEED_METERS_PER_FRAME;
        } else if idle {
            transform.translation.z = CAMERA_ORIGIN.z + (*frame as f32 * 0.012).sin() * 0.35;
        }
        if options.subtexel_motion {
            transform.translation.x = CAMERA_ORIGIN.x + (*frame % 8) as f32 * SUBTEXEL_STEP_METERS;
        } else if idle {
            transform.translation.x = CAMERA_ORIGIN.x + (*frame as f32 * 0.009).sin() * 0.45;
        }
        let target = transform.translation + CAMERA_TARGET_OFFSET;
        transform.look_at(target, Vec3::Y);
    }
}

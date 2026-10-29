//! Compare Moonly and native-math spot attenuation at fixed distances.

#[allow(dead_code)]
mod support;

use bevy::{
    app::AppExit, camera::Exposure, ecs::system::SystemParam, prelude::*,
    render::camera::CameraRenderGraph,
};
use moonly::prelude::*;
use support::{
    CaptureOptions, CaptureState, capture_system, deterministic_time,
    native_reference::setup_offscreen_capture, window_plugin,
};

#[derive(Resource)]
struct NativeMath(bool);

#[derive(SystemParam)]
struct SceneAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

const RANGE_METERS: f32 = 8.0;
const LIGHT_POSITION: Vec3 = Vec3::new(0.0, 0.45, 0.0);
const LIGHT_DIRECTION: Vec3 = Vec3::new(0.0, -0.1, 1.0);
const MARKER_FRACTIONS: [f32; 3] = [0.475, 0.95, 1.0];
const MARKER_COLORS: [Color; 3] = [
    Color::srgb(0.1, 0.85, 1.0),
    Color::srgb(1.0, 0.65, 0.08),
    Color::srgb(1.0, 0.12, 0.35),
];

fn main() -> AppExit {
    let native_math = std::env::args().any(|arg| arg == "--native-math");
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — Attenuation Lab")))
        .add_plugins((
            MoonlyCorePlugin,
            MoonlyCullPlugin,
            MoonlyShadowsPlugin,
            MoonlyGiPlugin,
            MoonlyRenderPlugin,
        ))
        .insert_resource(lab_config())
        .insert_resource(fixed_post_config())
        .insert_resource(dark_environment())
        .insert_resource(SunShaftsConfig {
            mode: SunShaftsMode::Off,
            ..SunShaftsConfig::default()
        })
        .insert_resource(ClearColor(Color::srgb(0.012, 0.016, 0.024)))
        .insert_resource(CaptureOptions::from_env_args(if native_math {
            "target/visual-audit/attenuation-native.png"
        } else {
            "target/visual-audit/attenuation-moonly.png"
        }))
        .insert_resource(CaptureState::default())
        .insert_resource(NativeMath(native_math))
        .insert_resource(deterministic_time())
        .add_systems(Startup, (setup_offscreen_capture, setup_scene).chain())
        .add_systems(Update, capture_system)
        .run()
}

fn lab_config() -> MoonlyConfig {
    let mut config = MoonlyConfig::moonly_r4();
    config.ao_mode = AmbientOcclusionMode::Off;
    config.gi.enabled = false;
    config.volumetrics.enabled = false;
    config
}

fn fixed_post_config() -> Hdr10PostConfig {
    Hdr10PostConfig {
        exposure: 1.0,
        exposure_mode: MoonlyExposureMode::Camera,
        contrast: 0.0,
        vibrance: 0.0,
        bloom_intensity: 0.0,
        flare_intensity: 0.0,
        ..Hdr10PostConfig::default()
    }
}

fn dark_environment() -> WeatherEnvironment {
    WeatherEnvironment {
        hemi_color: Vec4::ZERO,
        sun_color: Vec3::ZERO,
        ambient_color: Vec3::ZERO,
        fog_density: 0.0,
        fog_plane: Vec4::ZERO,
        sky_cube: [0.0; 6],
        ..WeatherEnvironment::default()
    }
}

fn setup_scene(
    mut commands: Commands,
    mut assets: SceneAssets,
    native_math: Res<NativeMath>,
    mut cameras: Query<(&mut Transform, &mut Exposure), With<MoonlyCamera>>,
) {
    spawn_ground(&mut commands, &mut assets.meshes, &mut assets.materials);
    spawn_markers(&mut commands, &mut assets.meshes, &mut assets.materials);
    spawn_spot(&mut commands, native_math.0);
    let transform =
        Transform::from_xyz(4.8, 7.8, 5.8).looking_at(Vec3::new(0.0, 0.0, 4.1), Vec3::Y);
    commands.spawn((
        MoonlyCamera::default(),
        CameraRenderGraph::new(MoonlyRender),
        Exposure { ev100: 0.0 },
        transform,
    ));
    for (mut camera_transform, mut exposure) in &mut cameras {
        *camera_transform = transform;
        exposure.ev100 = 0.0;
    }
}

fn spawn_ground(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    commands.spawn((
        Name::new("Attenuation ground"),
        Mesh3d(meshes.add(Cuboid::new(5.0, 0.12, RANGE_METERS + 1.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.34, 0.36, 0.4),
            perceptual_roughness: 0.82,
            ..StandardMaterial::default()
        })),
        Transform::from_xyz(0.0, -0.06, RANGE_METERS * 0.5),
    ));
}

fn spawn_markers(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let marker_mesh = meshes.add(Cuboid::new(4.2, 0.025, 0.085));
    for (index, (fraction, color)) in MARKER_FRACTIONS.into_iter().zip(MARKER_COLORS).enumerate() {
        commands.spawn((
            Name::new(format!("{fraction}R ground distance marker")),
            Mesh3d(marker_mesh.clone()),
            MeshMaterial3d(materials.add(marker_material(color))),
            Transform::from_xyz(0.0, 0.02, LIGHT_POSITION.z + RANGE_METERS * fraction),
        ));
        let height = 0.2 + index as f32 * 0.15;
        commands.spawn((
            Name::new(format!("{fraction}R marker tab")),
            Mesh3d(meshes.add(Cuboid::new(0.18, height, 0.18))),
            MeshMaterial3d(materials.add(marker_material(color))),
            Transform::from_xyz(
                -2.25,
                height * 0.5,
                LIGHT_POSITION.z + RANGE_METERS * fraction,
            ),
        ));
    }
}

fn marker_material(color: Color) -> StandardMaterial {
    let linear = color.to_linear();
    StandardMaterial {
        base_color: color,
        emissive: LinearRgba::new(linear.red * 4.0, linear.green * 4.0, linear.blue * 4.0, 1.0),
        perceptual_roughness: 1.0,
        ..StandardMaterial::default()
    }
}

fn spawn_spot(commands: &mut Commands, native_math: bool) {
    let flags = if native_math {
        MoonlyLightFlags::NATIVE
    } else {
        MoonlyLightFlags::default()
    };
    let direction = LIGHT_DIRECTION.normalize();
    commands.spawn((
        Name::new("Attenuation comparison spot"),
        MoonlyLight {
            light_type: MoonlyLightType::Spot,
            flags,
            color: Vec3::new(12.0, 10.5, 8.5),
            position: LIGHT_POSITION,
            direction,
            range: RANGE_METERS,
            cone_angle: 0.9,
            inner_cone_angle: 0.65,
            ..MoonlyLight::default()
        },
        Transform::from_translation(LIGHT_POSITION).looking_to(direction, Vec3::Y),
        GlobalTransform::default(),
    ));
}

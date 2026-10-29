#[allow(dead_code)]
mod support;

use bevy::{app::AppExit, prelude::*};
use moonly::{core::MoonlySystemSet, prelude::*};
use support::{
    CaptureOptions, CaptureState, capture_system, deterministic_time, float_arg, numeric_arg,
    window_plugin,
};

const WET_PLANE_HEIGHT: f32 = 0.02;
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct RainCamera;
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct RainOccluder {
    origin: Vec3,
    phase: f32,
}

const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

fn main() -> AppExit {
    let defaults = RainWetSurfaceConfig::default();
    let rain_wet_surface = RainWetSurfaceConfig {
        near_meters: float_arg("--rain-near", defaults.near_meters),
        far_meters: float_arg("--rain-far", defaults.far_meters),
        smap_resolution: numeric_arg("--rain-resolution", defaults.smap_resolution),
    };
    let config = MoonlyConfig {
        rain_wetness: 0.94,
        rain_wet_surface,
        ..MoonlyConfig::default()
    };
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — After Hours")))
        .add_plugins(MoonlyPlugin { config })
        .insert_resource(ClearColor(Color::srgb(0.035, 0.06, 0.11)))
        .insert_resource(night_environment())
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/rain-wet-shadow.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(deterministic_time())
        .add_systems(Startup, setup_rain_lab)
        .add_systems(
            Update,
            (animate_occluders, move_camera, capture_system).chain(),
        )
        .add_systems(
            PostUpdate,
            (draw_cpu_projection, log_rain_state).after(MoonlySystemSet::UpdateShadowsAndGi),
        )
        .run()
}

fn night_environment() -> WeatherEnvironment {
    WeatherEnvironment {
        hemi_color: Vec4::new(0.12, 0.20, 0.38, 0.42),
        ambient_color: v3(0.018, 0.025, 0.05),
        fog_color: v3(0.025, 0.055, 0.11),
        fog_density: 0.012,
        ..WeatherEnvironment::default()
    }
}
fn setup_rain_lab(
    mut commands: Commands,
    config: Res<MoonlyConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut post: ResMut<Hdr10PostConfig>,
) {
    post.operator = TonemapOperator::AcesHill;
    post.exposure = 1.15;
    post.bloom_intensity = 0.34;
    commands.spawn((MoonlyCamera::default(), RainCamera, camera_transform(0.0)));
    spawn_rooftop(&mut commands, &mut meshes, &mut materials);
    spawn_occluders(&mut commands, &mut meshes, &mut materials);
    spawn_lab_lights(&mut commands);
    let rain = &config.rain_wet_surface;
    info!(
        "Rain config: near={} far={} resolution={}; cinematic camera drift is continuous",
        rain.near_meters, rain.far_meters, rain.smap_resolution
    );
    info!(
        "Visual wet material + CPU projection diagnostic only; the GPU has no rain-shadow consumer"
    );
}
fn lit_material(
    materials: &mut Assets<StandardMaterial>,
    color: Vec3,
    roughness: f32,
    glow: Vec3,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: LinearRgba::new(color.x, color.y, color.z, 1.0).into(),
        metallic: 0.25,
        perceptual_roughness: roughness,
        emissive: LinearRgba::new(glow.x, glow.y, glow.z, 1.0),
        ..StandardMaterial::default()
    })
}

fn part(
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    transform: Transform,
) -> impl Bundle {
    (Mesh3d(mesh), MeshMaterial3d(material), transform)
}

fn spawn_rooftop(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let dark = lit_material(materials, v3(0.045, 0.065, 0.10), 0.4, Vec3::ZERO);
    let steel = lit_material(materials, v3(0.12, 0.16, 0.20), 0.16, Vec3::ZERO);
    let cyan = lit_material(materials, Vec3::ZERO, 0.2, v3(0.01, 2.4, 4.0));
    let pink = lit_material(materials, Vec3::ZERO, 0.2, v3(3.2, 0.015, 0.85));
    let ground = lit_material(materials, v3(0.035, 0.055, 0.085), 0.055, Vec3::ZERO);
    let ground_mesh = meshes.add(Cuboid::new(22.0, 0.16, 28.0));
    let ground_transform = Transform::from_xyz(0.0, WET_PLANE_HEIGHT - 0.08, -3.0);
    commands.spawn(part(ground_mesh, ground, ground_transform));
    for index in 0..6_u32 {
        let x = (index as f32 - 2.5) * 3.4;
        let height = 5.0 + (index % 3) as f32 * 2.2;
        let tower = meshes.add(Cuboid::new(2.6, height, 2.6));
        let transform = Transform::from_xyz(x, height * 0.5, -18.0);
        commands.spawn(part(tower, dark.clone(), transform));
        let material = if index.is_multiple_of(2) {
            cyan.clone()
        } else {
            pink.clone()
        };
        for floor in 0..4 {
            let window = meshes.add(Cuboid::new(1.2, 0.08, 0.04));
            let y = height * (0.25 + floor as f32 * 0.15);
            let transform = Transform::from_xyz(x, y, -16.68);
            commands.spawn(part(window, material.clone(), transform));
        }
    }
    for (center, size, material) in [
        (v3(-8.0, 0.65, -3.0), v3(0.5, 1.3, 24.0), dark.clone()),
        (v3(8.0, 0.65, -3.0), v3(0.5, 1.3, 24.0), dark.clone()),
        (v3(-5.8, 2.6, -9.0), v3(3.2, 5.2, 3.0), dark.clone()),
        (v3(5.6, 3.8, -12.0), v3(4.0, 7.6, 4.0), dark),
        (v3(5.6, 8.6, -12.0), v3(0.08, 2.0, 0.08), steel.clone()),
        (v3(5.6, 9.1, -12.0), v3(1.8, 0.06, 0.06), steel.clone()),
        (v3(-4.2, 0.55, -5.5), v3(1.7, 1.1, 1.3), steel.clone()),
        (v3(-4.5, 0.035, -2.0), v3(0.12, 0.025, 19.0), steel.clone()),
        (v3(4.5, 0.035, -2.0), v3(0.12, 0.025, 19.0), steel),
        (v3(-7.72, 1.0, -2.0), v3(0.06, 0.09, 15.0), cyan.clone()),
        (v3(-5.75, 3.4, -7.47), v3(1.8, 0.12, 0.05), cyan),
        (v3(7.72, 0.8, -4.0), v3(0.06, 0.09, 12.0), pink.clone()),
        (v3(5.6, 4.8, -9.97), v3(2.2, 0.12, 0.05), pink),
    ] {
        let mesh = meshes.add(Cuboid::from_size(size));
        commands.spawn(part(mesh, material, Transform::from_translation(center)));
    }
    let puddle_mesh = meshes.add(Cylinder::new(1.0, 0.008));
    let puddle = lit_material(materials, v3(0.035, 0.055, 0.085), 0.018, Vec3::ZERO);
    for (position, scale) in [
        (v3(-2.8, 0.027, 4.0), v3(2.8, 1.0, 1.1)),
        (v3(2.2, 0.027, 1.0), v3(1.7, 1.0, 2.9)),
    ] {
        let transform = Transform::from_translation(position).with_scale(scale);
        commands.spawn(part(puddle_mesh.clone(), puddle.clone(), transform));
    }
}

fn spawn_occluders(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let hub = meshes.add(Sphere::new(0.72).mesh().uv(24, 12));
    let fin = meshes.add(Cuboid::new(1.15, 0.12, 0.38));
    let metal = lit_material(materials, v3(0.30, 0.20, 0.12), 0.2, Vec3::ZERO);
    let glow = lit_material(materials, Vec3::ZERO, 0.15, v3(0.02, 0.8, 1.2));
    for (origin, phase) in [(v3(-2.8, 3.5, -1.0), 0.0), (v3(1.4, 4.4, -3.0), 2.1)] {
        commands
            .spawn((
                Transform::from_translation(origin),
                Visibility::default(),
                RainOccluder { origin, phase },
                MoonlyGiCaster::default(),
                MoonlyHomOccluder::default(),
            ))
            .with_children(|parent| {
                let scale = Transform::from_scale(v3(1.2, 0.42, 1.2));
                parent.spawn(part(hub.clone(), metal.clone(), scale));
                let core = Transform::from_scale(Vec3::splat(0.24));
                parent.spawn(part(hub.clone(), glow.clone(), core));
                for blade in 0..4 {
                    let angle = blade as f32 * std::f32::consts::FRAC_PI_2;
                    let transform = Transform::from_xyz(angle.cos(), 0.0, angle.sin())
                        .with_rotation(Quat::from_rotation_y(-angle));
                    parent.spawn(part(fin.clone(), metal.clone(), transform));
                }
            });
    }
}

fn spawn_lab_lights(commands: &mut Commands) {
    for (position, color) in [
        (v3(-6.5, 2.2, 0.0), v3(0.02, 1.5, 3.2)),
        (v3(6.3, 2.6, -4.0), v3(1.6, 0.012, 0.5)),
        (v3(0.0, 5.5, 5.0), v3(1.8, 0.65, 0.15)),
    ] {
        let light = MoonlyLight {
            color,
            position,
            range: 18.0,
            ..MoonlyLight::default()
        };
        commands.spawn((light, Transform::from_translation(position)));
    }
    commands.spawn((
        MoonlyLight {
            light_type: MoonlyLightType::Direct,
            color: v3(0.12, 0.20, 0.38),
            direction: v3(-0.3, -0.9, -0.2).normalize(),
            range: 60.0,
            ..MoonlyLight::default()
        },
        Transform::default(),
    ));
}

fn animate_occluders(time: Res<Time>, mut occluders: Query<(&mut Transform, &RainOccluder)>) {
    for (mut transform, occluder) in &mut occluders {
        let phase = time.elapsed_secs() * 0.65 + occluder.phase;
        transform.translation = occluder.origin
            + v3(
                phase.sin() * 1.3,
                (phase * 0.7).sin() * 0.35,
                phase.cos() * 0.8,
            );
        transform.rotation =
            Quat::from_rotation_y(phase) * Quat::from_rotation_z(phase.sin() * 0.12);
    }
}

fn camera_transform(seconds: f32) -> Transform {
    Transform::from_xyz(
        4.8 + (seconds * 0.20).sin() * 1.6,
        2.9 + (seconds * 0.27).sin() * 0.4,
        11.2 + (seconds * 0.16).cos() * 0.7,
    )
    .looking_at(v3(-0.6, 1.5, -3.8), Vec3::Y)
}

fn move_camera(time: Res<Time>, mut camera: Query<&mut Transform, With<RainCamera>>) {
    for mut transform in &mut camera {
        *transform = camera_transform(time.elapsed_secs());
    }
}

fn draw_cpu_projection(
    mut gizmos: Gizmos,
    rain: Res<RainShadowState>,
    occluders: Query<&Transform, With<RainOccluder>>,
) {
    for transform in &occluders {
        let Some(projected) = project_to_wet_plane(transform.translation, rain.rain_direction)
        else {
            continue;
        };
        gizmos.line(
            transform.translation,
            projected,
            Color::srgb(0.65, 0.38, 0.12),
        );
        let rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let isometry = Isometry3d::new(projected, rotation);
        gizmos.circle(isometry, 1.5, Color::srgb(0.02, 0.38, 0.48));
    }
}

fn project_to_wet_plane(start: Vec3, direction: Vec3) -> Option<Vec3> {
    if direction.y.abs() < f32::EPSILON {
        return None;
    }
    let distance = (WET_PLANE_HEIGHT - start.y) / direction.y;
    (distance >= 0.0).then(|| start + direction * distance)
}

fn log_rain_state(rain: Res<RainShadowState>, mut frame: Local<u32>) {
    *frame += 1;
    if *frame == 45 {
        info!(
            "RainShadowState frame 45: center={:?} radius={:.4} generation={} smap_size={}",
            rain.sphere_center, rain.sphere_radius, rain.generation, rain.smap_size
        );
    }
}

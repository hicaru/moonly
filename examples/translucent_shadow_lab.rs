#[allow(dead_code)]
mod support;

use bevy::{
    app::AppExit, camera::Exposure, pbr::DistanceFog, prelude::*, render::camera::CameraRenderGraph,
};
use moonly::prelude::*;
use support::{
    CaptureOptions, CaptureState, capture_system, deterministic_time, native_reference,
    window_plugin,
};

#[derive(Component)]
struct GlassPanel {
    phase: f32,
}

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — Translucent Shadow Gallery")))
        .add_plugins((
            MoonlyCorePlugin,
            MoonlyCullPlugin,
            MoonlyShadowsPlugin,
            MoonlyGiPlugin,
            MoonlyRenderPlugin,
        ))
        .insert_resource(MoonlyConfig::moonly_r4())
        .insert_resource(Hdr10PostConfig {
            exposure_mode: MoonlyExposureMode::Camera,
            ..Hdr10PostConfig::default()
        })
        .insert_resource(WeatherEnvironment {
            fog_density: 0.006,
            ..WeatherEnvironment::default()
        })
        .insert_resource(ClearColor(Color::srgb(0.055, 0.075, 0.12)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/translucent-gallery.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(deterministic_time())
        .add_systems(
            Startup,
            (
                native_reference::setup_offscreen_capture,
                setup_glass_gallery,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (animate_gallery, report_surface_probe, capture_system).chain(),
        )
        .run()
}

fn setup_glass_gallery(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor = materials.add(StandardMaterial {
        base_color: Color::srgb(0.52, 0.55, 0.52),
        perceptual_roughness: 0.48,
        ..StandardMaterial::default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(14.0)))),
        MeshMaterial3d(floor),
    ));
    spawn_glass_panels(&mut commands, &mut meshes, &mut materials);
    spawn_frame_and_lights(&mut commands, &mut meshes, &mut materials);
    commands.spawn((
        MoonlyCamera::default(),
        support::reference_scene::debug_view_from_args(),
        CameraRenderGraph::new(MoonlyRender),
        Exposure { ev100: 0.0 },
        DistanceFog::default(),
        Transform::from_xyz(5.8, 4.2, 8.5).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));
}

fn spawn_glass_panels(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let colors = [
        Vec4::new(0.95, 0.12, 0.04, 0.88),
        Vec4::new(0.08, 0.55, 0.95, 0.82),
        Vec4::new(0.95, 0.65, 0.08, 0.86),
    ];
    for (index, transmittance) in colors.into_iter().enumerate() {
        let tint = Vec3::new(0.3, 0.5, 0.8) * transmittance.truncate();
        let material = materials.add(StandardMaterial {
            base_color: Color::linear_rgb(tint.x, tint.y, tint.z),
            perceptual_roughness: 0.2,
            metallic: 0.3,
            ..StandardMaterial::default()
        });
        let x = (index as f32 - 1.0) * 1.45;
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(1.2, 2.3, 0.12))),
            MeshMaterial3d(material),
            Transform::from_xyz(x, 1.55, 0.6),
            MoonlySurface {
                transmittance: if std::env::args().any(|arg| arg == "--opaque") {
                    Vec4::new(0.0, 0.0, 0.0, 1.0)
                } else {
                    transmittance
                },
                ..MoonlySurface::default()
            },
            GlassPanel {
                phase: index as f32 * 0.7,
            },
        ));
    }
}

fn spawn_frame_and_lights(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let frame = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.10, 0.08),
        metallic: 0.8,
        perceptual_roughness: 0.26,
        ..StandardMaterial::default()
    });
    let rail = meshes.add(Cuboid::new(4.8, 0.12, 0.22));
    for y in [0.35, 2.75] {
        commands.spawn((
            Mesh3d(rail.clone()),
            MeshMaterial3d(frame.clone()),
            Transform::from_xyz(0.0, y, 0.6),
        ));
    }
    commands.spawn(MoonlyLight::spot(
        Vec3::new(-2.8, 7.0, 4.5),
        Vec3::new(0.25, -1.0, -0.45).normalize(),
        Vec3::new(9.0, 7.0, 4.8),
        20.0,
        1.25,
    ));
    let rim = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::new(0.4, 0.7, 1.6, 1.0),
        ..StandardMaterial::default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Torus::new(3.4, 3.48))),
        MeshMaterial3d(rim),
        Transform::from_xyz(0.0, 0.025, -1.0).with_rotation(Quat::from_rotation_x(1.57)),
    ));
}

fn report_surface_probe(
    surfaces: Query<&MoonlySurface>,
    package: Res<LightPackage>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    if *frame != 45 || !std::env::args().any(|arg| arg == "--surface-probe") {
        return;
    }
    for surface in &surfaces {
        info!("Surface transmittance: {:?}", surface.transmittance);
    }
    info!("Shadowed lights: {}", package.v_shadowed.len());
}

fn animate_gallery(
    time: Res<Time>,
    mut panels: Query<(&mut Transform, &GlassPanel), Without<MoonlyCamera>>,
    mut cameras: Query<&mut Transform, With<MoonlyCamera>>,
) {
    if std::env::args().any(|arg| arg == "--static") {
        return;
    }
    let seconds = time.elapsed_secs();
    for (mut transform, panel) in &mut panels {
        transform.rotation = Quat::from_rotation_y((seconds * 0.35 + panel.phase).sin() * 0.28);
    }
    for mut transform in &mut cameras {
        transform.translation = Vec3::new(5.8 + seconds.sin() * 0.35, 4.2, 8.5);
        transform.look_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y);
    }
}

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
struct RevealOccluder;

#[derive(Component)]
struct HoverOrb {
    origin: Vec3,
    phase: f32,
}

#[derive(bevy::ecs::system::SystemParam)]
struct GalleryAnimation<'w, 's> {
    time: Res<'w, Time>,
    occluders: Query<'w, 's, Entity, With<RevealOccluder>>,
    orbs: Query<'w, 's, (&'static mut Transform, &'static HoverOrb), Without<MoonlyCamera>>,
    cameras: Query<'w, 's, &'static mut Transform, With<MoonlyCamera>>,
}

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — HOM Night Gallery")))
        .add_plugins((
            MoonlyCorePlugin,
            MoonlyCullPlugin,
            MoonlyShadowsPlugin,
            MoonlyGiPlugin,
            MoonlyRenderPlugin,
        ))
        .insert_resource(MoonlyConfig::moonly_r4())
        .insert_resource(MoonlyHomCacheConfig {
            enabled: !std::env::args().any(|arg| arg == "--no-hom-cache"),
        })
        .insert_resource(Hdr10PostConfig {
            exposure_mode: MoonlyExposureMode::Camera,
            ..Hdr10PostConfig::default()
        })
        .insert_resource(WeatherEnvironment {
            fog_density: 0.009,
            fog_height_falloff: 0.06,
            ..WeatherEnvironment::default()
        })
        .insert_resource(ClearColor(Color::srgb(0.07, 0.10, 0.17)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/hom-temporal-lab.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(deterministic_time())
        .add_systems(
            Startup,
            (native_reference::setup_offscreen_capture, setup_gallery).chain(),
        )
        .add_systems(
            Update,
            (animate_gallery, report_hom_counters, capture_system).chain(),
        )
        .run()
}

fn setup_gallery(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_floor(&mut commands, &mut meshes, &mut materials);
    spawn_night_sky(&mut commands, &mut meshes, &mut materials);
    spawn_gallery_panels(&mut commands, &mut meshes, &mut materials);
    spawn_gallery_orbs(&mut commands, &mut meshes, &mut materials);
    spawn_gallery_lights(&mut commands);
    commands.spawn((
        MoonlyCamera::default(),
        CameraRenderGraph::new(MoonlyRender),
        Exposure { ev100: 0.0 },
        DistanceFog::default(),
        Transform::from_xyz(4.8, 4.2, 13.5).looking_at(Vec3::new(0.0, 1.2, -2.0), Vec3::Y),
    ));
}

fn spawn_floor(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let floor = materials.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.14, 0.18),
        perceptual_roughness: 0.28,
        metallic: 0.25,
        ..StandardMaterial::default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(22.0)))),
        MeshMaterial3d(floor),
    ));
    let inlay = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.22, 0.12),
        emissive: LinearRgba::new(0.24, 0.12, 0.025, 1.0),
        perceptual_roughness: 0.4,
        ..StandardMaterial::default()
    });
    let strip = meshes.add(Cuboid::new(0.055, 0.015, 18.0));
    for x in [-3.0, 3.0] {
        commands.spawn((
            Mesh3d(strip.clone()),
            MeshMaterial3d(inlay.clone()),
            Transform::from_xyz(x, 0.012, -1.0),
        ));
    }
}

fn spawn_night_sky(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let sky = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::new(0.035, 0.055, 0.11, 1.0),
        unlit: true,
        ..StandardMaterial::default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(200.0, 100.0, 0.2))),
        MeshMaterial3d(sky),
        Transform::from_xyz(0.0, 20.0, -25.0),
    ));
    let moon = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::new(2.2, 1.8, 1.1, 1.0),
        unlit: true,
        ..StandardMaterial::default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.35))),
        MeshMaterial3d(moon),
        Transform::from_xyz(-6.0, 8.0, -20.0),
    ));
}

fn spawn_gallery_panels(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let panel = meshes.add(Cuboid::new(1.55, 3.4, 0.28));
    let cap = meshes.add(Cuboid::new(1.7, 0.09, 0.42));
    let bronze = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.20, 0.12),
        metallic: 0.65,
        perceptual_roughness: 0.32,
        ..StandardMaterial::default()
    });
    let light = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::new(1.8, 0.72, 0.16, 1.0),
        ..StandardMaterial::default()
    });
    for row in 0..3 {
        for column in -3..=3 {
            let position = Vec3::new(column as f32 * 2.4, 1.7, -row as f32 * 1.5);
            let mut entity = commands.spawn((
                Mesh3d(panel.clone()),
                MeshMaterial3d(bronze.clone()),
                Transform::from_translation(position),
                MoonlyHomOccluder { double_sided: true },
            ));
            if column == 0 {
                entity.insert(RevealOccluder);
            }
            commands.spawn((
                Mesh3d(cap.clone()),
                MeshMaterial3d(light.clone()),
                Transform::from_translation(position + Vec3::Y * 1.74),
            ));
        }
    }
}

fn spawn_gallery_orbs(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let mesh = meshes.add(Sphere::new(0.62));
    let colors = [
        Vec3::new(2.8, 0.28, 0.08),
        Vec3::new(0.08, 1.4, 3.2),
        Vec3::new(0.5, 2.4, 0.3),
    ];
    for (index, x) in [-4.0, 0.0, 4.0].into_iter().enumerate() {
        let energy = colors.get(index).copied().unwrap_or(Vec3::ONE);
        let material = materials.add(StandardMaterial {
            base_color: Color::BLACK,
            emissive: LinearRgba::new(energy.x, energy.y, energy.z, 1.0),
            perceptual_roughness: 0.18,
            ..StandardMaterial::default()
        });
        let z = if index == 1 { -2.2 } else { -4.2 };
        let origin = Vec3::new(x, 1.3, z);
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(origin),
            HoverOrb {
                origin,
                phase: index as f32 * 2.1,
            },
        ));
        commands.spawn(MoonlyLight {
            color: energy * 1.8,
            position: origin,
            range: 8.0,
            ..MoonlyLight::default()
        });
    }
}

fn spawn_gallery_lights(commands: &mut Commands) {
    commands.spawn(MoonlyLight::spot(
        Vec3::new(2.0, 8.0, 9.0),
        Vec3::new(-0.2, -0.65, -1.0).normalize(),
        Vec3::new(7.0, 4.5, 2.8),
        28.0,
        1.25,
    ));
}

fn animate_gallery(mut commands: Commands, inputs: GalleryAnimation, mut frame: Local<u32>) {
    let GalleryAnimation {
        time,
        occluders,
        mut orbs,
        mut cameras,
    } = inputs;
    *frame += 1;
    if *frame == 35 && std::env::args().any(|arg| arg == "--hom-reveal") {
        for entity in &occluders {
            commands.entity(entity).despawn();
        }
    }
    let seconds = time.elapsed_secs();
    for (mut transform, orb) in &mut orbs {
        transform.translation = orb.origin + Vec3::Y * (seconds * 1.4 + orb.phase).sin() * 0.22;
        transform.rotate_y(0.008);
    }
    let travel = if std::env::args().any(|arg| arg == "--hom-motion") {
        seconds * 0.35
    } else {
        seconds * 0.08
    };
    for mut transform in &mut cameras {
        transform.translation = Vec3::new(4.8 + travel.sin() * 1.4, 4.2, 13.5 - travel * 0.35);
        transform.look_at(Vec3::new(0.0, 1.2, -2.0), Vec3::Y);
    }
}

fn report_hom_counters(diagnostics: Res<MoonlyHomDiagnostics>, mut frame: Local<u32>) {
    *frame += 1;
    if *frame != 45 && *frame != 90 {
        return;
    }
    info!(
        "HOM frame {}: {} us, rasterized {}, rejected {}, skipped {}, reused {}, tested {}",
        *frame,
        diagnostics.update_micros,
        diagnostics.rasterized,
        diagnostics.rejected,
        diagnostics.skipped,
        diagnostics.visibility_reused,
        diagnostics.visibility_tested
    );
}

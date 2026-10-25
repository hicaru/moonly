#[allow(dead_code)]
mod support;

use bevy::{app::AppExit, prelude::*};
use moonly::{core::MoonlySystemSet, prelude::*};
use support::{
    CaptureOptions, CaptureState, capture_system, deterministic_time,
    scene::{spawn_camera, spawn_test_scene},
    window_plugin,
};

#[derive(Component)]
struct StressLight(u32);

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — Light Lifecycle Stress")))
        .add_plugins(MoonlyPlugin::enhanced())
        .insert_resource(ClearColor(Color::srgb(0.06, 0.08, 0.12)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/lighting-stress.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(deterministic_time())
        .add_systems(Startup, setup_stress)
        .add_systems(Update, (mutate_lights, capture_system).chain())
        .add_systems(
            PostUpdate,
            audit_light_state.after(MoonlySystemSet::SyncMaterials),
        )
        .run()
}

fn setup_stress(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
    mut config: ResMut<MoonlyConfig>,
) {
    config.smap_pool_size = 1024;
    spawn_camera(&mut commands);
    spawn_test_scene(&mut commands, &mut meshes, &mut materials);
    for index in 0..12_u32 {
        let angle = index as f32 * std::f32::consts::TAU / 12.0;
        let position = Vec3::new(angle.cos() * 3.0, 2.0 + index as f32 * 0.08, angle.sin());
        let mut light = MoonlyLight {
            flags: MoonlyLightFlags::GI_ENABLED,
            color: Vec3::new(0.4 + index as f32 * 0.05, 0.7, 1.2),
            range: 8.0,
            ..MoonlyLight::default()
        };
        light.position = position;
        commands.spawn((
            light,
            StressLight(index),
            Transform::from_translation(position),
            GlobalTransform::default(),
        ));
    }
    let mut direct = MoonlyLight {
        light_type: MoonlyLightType::Direct,
        flags: MoonlyLightFlags::GI_ENABLED,
        direction: Vec3::new(-0.4, -0.8, -0.3).normalize(),
        range: 100.0,
        ..MoonlyLight::default()
    };
    direct.position = Vec3::ZERO;
    commands.spawn((direct, StressLight(100), Transform::default()));
}

fn mutate_lights(
    mut commands: Commands,
    mut lights: Query<(Entity, &StressLight, &mut Transform)>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    for (entity, marker, mut transform) in &mut lights {
        if marker.0 == 0 && *frame >= 90 {
            transform.translation = Vec3::ZERO;
        } else if marker.0 < 12 {
            transform.rotation = Quat::from_rotation_y(*frame as f32 * 0.01);
        }
        if marker.0 == 2 && *frame == 150 {
            commands.entity(entity).despawn();
        }
    }
}

fn audit_light_state(
    package: Res<LightPackage>,
    gi: Res<GiLightField>,
    allocator: Res<SmapAllocator>,
    mut exit: MessageWriter<AppExit>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    let valid = package
        .v_point
        .iter()
        .chain(&package.v_spot)
        .chain(&package.v_shadowed)
        .chain(&package.v_direct)
        .all(|light| {
            light.position.is_finite()
                && light.direction.is_finite()
                && light.right.is_finite()
                && light.range >= 0.0
        });
    let origin_synced = *frame < 90
        || package
            .v_point
            .iter()
            .any(|light| light.position == Vec3::ZERO);
    let despawn_synced = *frame < 150 || package.v_point.len() == 11;
    if !valid || !origin_synced || !despawn_synced || package.v_direct.len() != 1 {
        error!("Invalid light package during stress test");
        exit.write(AppExit::error());
    }
    if (*frame).is_multiple_of(60) {
        info!(
            "Stress frame {}: points {}, spots {}, shadowed {}, direct {}, GI {}, atlas {}",
            *frame,
            package.v_point.len(),
            package.v_spot.len(),
            package.v_shadowed.len(),
            package.v_direct.len(),
            gi.virtual_lights.len(),
            allocator.allocated.len()
        );
    }
}

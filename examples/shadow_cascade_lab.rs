#[allow(dead_code)]
mod support;

use bevy::{app::AppExit, prelude::*};
use moonly::{core::MoonlySystemSet, prelude::*};
use support::{
    CaptureOptions, CaptureState, capture_system, deterministic_time, scene::spawn_test_scene,
    window_plugin,
};

#[derive(Component)]
struct CascadeCamera;

struct CameraFrame {
    transform: GlobalTransform,
    tan_half_fov: f32,
    aspect: f32,
}

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin("Moonly — CPU Cascade Diagnostics")))
        .add_plugins(MoonlyPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.07, 0.09, 0.13)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/shadow-cascades.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(deterministic_time())
        .add_systems(Startup, setup_cascades)
        .add_systems(Update, (move_camera, capture_system).chain())
        .add_systems(
            PostUpdate,
            (draw_cascade_gizmos, audit_cascades)
                .chain()
                .after(MoonlySystemSet::UpdateShadowsAndGi),
        )
        .run()
}

fn setup_cascades(
    mut commands: Commands,
    mut config: ResMut<MoonlyConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
) {
    config.sun_cascades.near_split = 2.0;
    config.sun_cascades.middle_split = 4.0;
    config.sun_cascades.far_split = 9.0;
    commands.spawn((
        Camera3d::default(),
        CascadeCamera,
        Transform::from_xyz(0.0, 2.6, 7.2).looking_at(Vec3::new(0.0, 1.1, 0.0), Vec3::Y),
    ));
    spawn_test_scene(&mut commands, &mut meshes, &mut materials);
    info!("Gizmos show CPU cascade splits. No GPU shadow atlas pass is implied.");
}

fn move_camera(mut camera: Query<&mut Transform, With<CascadeCamera>>, mut frame: Local<u32>) {
    *frame += 1;
    for mut transform in &mut camera {
        let phase = *frame as f32 * 0.008;
        *transform = Transform::from_xyz(phase.sin() * 1.5, 2.6, 7.2 + phase.cos() * 0.3)
            .looking_at(Vec3::new(0.0, 1.1, 0.0), Vec3::Y);
    }
}

fn draw_cascade_gizmos(
    mut gizmos: Gizmos,
    camera: Query<(&GlobalTransform, &Projection), With<CascadeCamera>>,
    cascades: Res<SunCascades>,
) {
    let Ok((transform, projection)) = camera.single() else {
        return;
    };
    let Projection::Perspective(perspective) = projection else {
        return;
    };
    let frame = CameraFrame {
        transform: *transform,
        tan_half_fov: (perspective.fov * 0.5).tan(),
        aspect: perspective.aspect_ratio,
    };
    for (slice, color) in cascades.cascades.iter().zip([
        Color::srgb(1.0, 0.2, 0.2),
        Color::srgb(0.2, 1.0, 0.3),
        Color::srgb(0.2, 0.5, 1.0),
    ]) {
        draw_frustum_square(&mut gizmos, &frame, slice.split_near, color);
        draw_frustum_square(&mut gizmos, &frame, slice.split_far, color);
    }
}

fn draw_frustum_square(gizmos: &mut Gizmos, frame: &CameraFrame, distance: f32, color: Color) {
    let center = frame.transform.translation() + frame.transform.forward() * distance;
    let half_y = distance * frame.tan_half_fov;
    let half_x = half_y * frame.aspect;
    let right = frame.transform.right() * half_x;
    let up = frame.transform.up() * half_y;
    let corners = [
        center - right - up,
        center + right - up,
        center + right + up,
        center - right + up,
    ];
    for index in 0..4 {
        let Some(from) = corners.get(index).copied() else {
            continue;
        };
        let Some(to) = corners.get((index + 1) % 4).copied() else {
            continue;
        };
        gizmos.line(from, to, color);
    }
}

fn audit_cascades(
    cascades: Res<SunCascades>,
    allocator: Res<SmapAllocator>,
    mut exit: MessageWriter<AppExit>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    let expected = [(0.1, 2.0), (2.0, 4.0), (4.0, 9.0)];
    let splits_valid = cascades
        .cascades
        .iter()
        .zip(expected)
        .all(|(slice, (near, far))| {
            (slice.split_near - near).abs() < 1.0e-5 && (slice.split_far - far).abs() < 1.0e-5
        });
    if !splits_valid
        || cascades
            .cascades
            .iter()
            .any(|slice| !slice.combine.is_finite() || slice.split_far <= slice.split_near)
    {
        error!("Invalid cascade matrix or split");
        exit.write(AppExit::error());
    }
    if (*frame).is_multiple_of(60) {
        for (index, slice) in cascades.cascades.iter().enumerate() {
            info!(
                "Cascade {index}: {}..{} m, determinant {}, atlas rects {}",
                slice.split_near,
                slice.split_far,
                slice.combine.determinant(),
                allocator.allocated.len()
            );
        }
    }
}

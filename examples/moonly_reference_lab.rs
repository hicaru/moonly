#[allow(dead_code)]
mod support;

use bevy::{app::AppExit, prelude::*, render::camera::CameraRenderGraph, window::WindowResolution};
use moonly_core::{AmbientOcclusionMode, LightPackage, MoonlyConfig, MoonlyCorePlugin};
use moonly_cull::MoonlyCullPlugin;
use moonly_gi::MoonlyGiPlugin;
use moonly_post::LuminanceAdaptationState;
use moonly_render::{
    GpuTiming, MoonlyCamera, MoonlyDebugView, MoonlyGpuDiagnostics, MoonlyLinearOutput, MoonlyPass,
    MoonlyRender, MoonlyRenderPlugin,
};
use moonly_shadows::MoonlyShadowsPlugin;
use moonly_volumetrics::{SunShaftsConfig, SunShaftsMode};
use support::{
    CaptureOptions, CaptureState, capture_system,
    native_reference::setup_offscreen_capture,
    reference_scene::{debug_view_from_args, move_cascade_camera, move_caster, spawn_scene},
};

fn main() -> AppExit {
    App::new()
        .add_plugins(DefaultPlugins.set(support::window_plugin("Moonly Reference Lab")))
        .add_plugins((
            MoonlyCorePlugin,
            MoonlyCullPlugin,
            MoonlyShadowsPlugin,
            MoonlyGiPlugin,
            MoonlyRenderPlugin,
        ))
        .insert_resource(MoonlyConfig {
            ao_mode: if std::env::args().any(|arg| arg == "--no-ao") {
                AmbientOcclusionMode::Off
            } else {
                AmbientOcclusionMode::Hdao
            },
            rain_wetness: if std::env::args().any(|arg| arg == "--wet") {
                1.0
            } else {
                0.0
            },
            ..MoonlyConfig::moonly_r4()
        })
        .insert_resource(moonly_core::WeatherEnvironment {
            fog_density: if std::env::args().any(|arg| arg == "--fog" || arg == "--fog-transparent")
            {
                0.03
            } else {
                0.0
            },
            fog_height_falloff: 0.08,
            fog_height_reference: 0.0,
            ..moonly_core::WeatherEnvironment::default()
        })
        .insert_resource(moonly_volumetrics::MoonlyFroxelConfig {
            enabled: std::env::args().any(|arg| arg == "--froxel"),
            ..moonly_volumetrics::MoonlyFroxelConfig::default()
        })
        .insert_resource(SunShaftsConfig {
            mode: if std::env::args().any(|arg| arg == "--screen-shafts") {
                SunShaftsMode::ScreenSpace
            } else {
                SunShaftsMode::Combined
            },
            ..SunShaftsConfig::default()
        })
        .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.05)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/moonly-reference.png",
        ))
        .insert_resource(CaptureState::default())
        .insert_resource(support::deterministic_time())
        .add_systems(Startup, (setup_offscreen_capture, spawn_scene).chain())
        .add_systems(
            Update,
            (
                exercise_view_lifecycle,
                move_caster,
                move_cascade_camera,
                report_exposure_probe,
                report_gpu_probe,
                capture_system,
            )
                .chain(),
        )
        .run()
}

fn report_exposure_probe(
    state: Res<LuminanceAdaptationState>,
    package: Res<LightPackage>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    if *frame == 45 && std::env::args().any(|arg| arg == "--exposure-probe") {
        info!(
            "Measured luminance {:.6}, adapted {:.6}, exposure {:.6}",
            state.target_luminance,
            state.adapted_luminance,
            state.compute_exposure_scale()
        );
        info!(
            "Lights: point {}, spot {}, shadowed {}, direct {}",
            package.v_point.len(),
            package.v_spot.len(),
            package.v_shadowed.len(),
            package.v_direct.len()
        );
    }
}

fn report_gpu_probe(diagnostics: Res<MoonlyGpuDiagnostics>, mut frame: Local<u32>) {
    *frame += 1;
    if *frame != 45 || !std::env::args().any(|arg| arg == "--gpu-probe") {
        return;
    }
    for pass in MoonlyPass::ALL {
        match diagnostics.timing(pass) {
            GpuTiming::Available(duration) => {
                info!(
                    "GPU {}: {:.3} ms",
                    pass.label(),
                    duration.as_secs_f64() * 1000.0
                );
            }
            GpuTiming::Pending => info!("GPU {}: pending", pass.label()),
            GpuTiming::Unavailable => info!("GPU {}: unavailable", pass.label()),
        }
    }
}

fn spawn_moonly_camera(commands: &mut Commands, debug_view: MoonlyDebugView) {
    let mut camera = commands.spawn((
        MoonlyCamera::default(),
        debug_view,
        CameraRenderGraph::new(MoonlyRender),
        Transform::from_xyz(5.5, 4.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
    if std::env::args().any(|arg| arg == "--linear") {
        camera.insert(MoonlyLinearOutput);
    }
}

fn exercise_view_lifecycle(
    mut commands: Commands,
    mut windows: Query<&mut Window>,
    cameras: Query<Entity, With<MoonlyCamera>>,
    mut frame: Local<u32>,
) {
    if !std::env::args().any(|arg| arg == "--lifecycle") {
        return;
    }
    *frame += 1;
    if *frame == 10 {
        for mut window in &mut windows {
            window.resolution = WindowResolution::new(960, 540);
        }
    }
    if *frame == 20 {
        for entity in &cameras {
            commands.entity(entity).despawn();
        }
    }
    if *frame == 30 {
        spawn_moonly_camera(&mut commands, debug_view_from_args());
    }
}

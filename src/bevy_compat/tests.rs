use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    post_process::bloom::Bloom,
    render::{camera::CameraRenderGraph, view::ColorGrading},
};
use moonly_core::{LightPackage, MoonlyLight};
use moonly_post::TonemapOperator;
use moonly_render::MoonlyRender;

use super::*;

fn compat_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MoonlyBevyCompatPlugin));
    app
}

#[test]
fn automatic_camera_setup_supports_backfill_and_opt_out() {
    let mut app = App::new();
    let existing = app.world_mut().spawn(Camera3d::default()).id();
    let opted_out = app
        .world_mut()
        .spawn((Camera3d::default(), MoonlyBevyCompatOptOut))
        .id();
    app.add_plugins((MinimalPlugins, MoonlyBevyCompatPlugin));
    let observed = app.world_mut().spawn(Camera3d::default()).id();

    app.update();
    assert!(app.world().get::<MoonlyCamera>(observed).is_some());

    for entity in [existing, observed] {
        assert!(app.world().get::<MoonlyCamera>(entity).is_some());
        let settings = app.world().get::<MoonlyPostSettings>(entity);
        assert!(settings.is_some_and(|settings| {
            settings.exposure_mode == MoonlyExposureMode::Camera && settings.native_post
        }));
    }
    assert!(app.world().get::<MoonlyCamera>(opted_out).is_none());
    assert!(app.world().get::<MoonlyPostSettings>(opted_out).is_none());
}

#[test]
fn backfill_does_not_turn_scene_entities_into_cameras() {
    let mut app = App::new();
    let mesh = app.world_mut().spawn(Mesh3d::default()).id();
    let light = app.world_mut().spawn(PointLight::default()).id();
    app.add_plugins((MinimalPlugins, MoonlyBevyCompatPlugin));
    app.update();

    for entity in [mesh, light] {
        assert!(app.world().get::<MoonlyCamera>(entity).is_none());
        assert!(app.world().get::<Camera3d>(entity).is_none());
        assert!(app.world().get::<MoonlyPostSettings>(entity).is_none());
    }
}

#[test]
fn native_bloom_synchronizes_removal_and_reinsertion() {
    let mut app = compat_app();
    let camera = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Bloom {
                intensity: 0.4,
                prefilter: bevy::post_process::bloom::BloomPrefilter {
                    threshold: 2.5,
                    threshold_softness: 0.3,
                },
                ..Bloom::default()
            },
        ))
        .id();
    app.update();
    assert_bloom(&app, camera, 0.4, 2.5, 0.3);

    app.world_mut().entity_mut(camera).remove::<Bloom>();
    app.update();
    let defaults = MoonlyPostSettings::default();
    assert_bloom(
        &app,
        camera,
        defaults.bloom_intensity,
        defaults.bloom_threshold,
        defaults.bloom_softness,
    );

    app.world_mut().entity_mut(camera).insert(Bloom {
        intensity: 0.7,
        prefilter: bevy::post_process::bloom::BloomPrefilter {
            threshold: 4.0,
            threshold_softness: 0.6,
        },
        ..Bloom::default()
    });
    app.update();
    assert_bloom(&app, camera, 0.7, 4.0, 0.6);
    assert!(app.world().get::<Bloom>(camera).is_some());
}

fn assert_bloom(app: &App, camera: Entity, intensity: f32, threshold: f32, softness: f32) {
    let values = app
        .world()
        .get::<MoonlyPostSettings>(camera)
        .map(|settings| {
            (
                settings.bloom_intensity,
                settings.bloom_threshold,
                settings.bloom_softness,
            )
        });
    assert_eq!(values, Some((intensity, threshold, softness)));
}

#[test]
fn native_tonemapping_and_grading_remain_on_the_camera() {
    let mut app = compat_app();
    let mut grading = ColorGrading::default();
    grading.global.exposure = 1.5;
    grading.global.temperature = 0.2;
    grading.global.tint = -0.1;
    grading.global.post_saturation = 1.3;
    let camera = app
        .world_mut()
        .spawn((Camera3d::default(), Tonemapping::AcesFitted, grading))
        .id();
    app.update();

    assert_eq!(
        app.world().get::<Tonemapping>(camera),
        Some(&Tonemapping::AcesFitted)
    );
    let settings = app.world().get::<MoonlyPostSettings>(camera);
    assert!(settings.is_some_and(|settings| {
        settings.operator == TonemapOperator::AcesHill
            && (settings.grading_exposure - 1.5).abs() < f32::EPSILON
            && (settings.temperature - 0.2).abs() < f32::EPSILON
            && (settings.tint + 0.1).abs() < f32::EPSILON
            && (settings.saturation - 1.3).abs() < f32::EPSILON
    }));
    assert!(app.world().get::<ColorGrading>(camera).is_some());

    app.world_mut().entity_mut(camera).insert(Tonemapping::AgX);
    app.update();
    assert_eq!(
        app.world().get::<Tonemapping>(camera),
        Some(&Tonemapping::AgX)
    );
    assert!(
        app.world()
            .get::<MoonlyPostSettings>(camera)
            .is_some_and(|settings| settings.operator == TonemapOperator::AgX)
    );
}

#[test]
fn main_plugin_adapts_native_cameras_without_a_compatibility_plugin() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MoonlyPlugin::default()));
    let camera = app.world_mut().spawn(Camera3d::default()).id();
    app.update();

    assert!(app.world().get::<MoonlyCamera>(camera).is_some());
    assert!(app.world().resource::<MoonlyNativeAmbient>().0);
    assert!(!app.world().resource::<MoonlyNativeForward>().0);
    assert!(app.world().contains_resource::<LightPackage>());
    let graph = app.world().get::<CameraRenderGraph>(camera);
    assert!(graph.is_some_and(|graph| graph.0 == CameraRenderGraph::new(MoonlyRender).0));
}

#[test]
fn explicit_native_forward_configuration_is_preserved() {
    let mut app = App::new();
    app.insert_resource(MoonlyNativeForward(true))
        .insert_resource(MoonlyNativePost(true))
        .insert_resource(MoonlyNativeAmbient(false));
    app.add_plugins((MinimalPlugins, MoonlyPlugin::default()));
    let camera = app.world_mut().spawn(Camera3d::default()).id();
    app.update();

    assert!(app.world().resource::<MoonlyNativeForward>().0);
    assert!(app.world().resource::<MoonlyNativePost>().0);
    assert!(!app.world().resource::<MoonlyNativeAmbient>().0);
    assert!(!app.world().contains_resource::<LightPackage>());
    let graph = app.world().get::<CameraRenderGraph>(camera);
    assert!(graph.is_some_and(
        |graph| graph.0 == CameraRenderGraph::new(bevy::core_pipeline::schedule::Core3d).0
    ));
}

#[test]
fn explicit_moonly_camera_keeps_global_post_settings() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MoonlyPlugin::enhanced()));
    let camera = app
        .world_mut()
        .spawn((Camera3d::default(), MoonlyCamera::default()))
        .id();
    app.update();

    assert!(app.world().get::<MoonlyPostSettings>(camera).is_none());
}

#[test]
fn explicit_moonly_lights_keep_moonly_exposure_and_post() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MoonlyPlugin::enhanced()));
    let camera = app
        .world_mut()
        .spawn((Camera3d::default(), Bloom::NATURAL))
        .id();
    app.world_mut().spawn(MoonlyLight::default());
    app.update();

    let settings = app.world().get::<MoonlyPostSettings>(camera);
    assert!(settings.is_some_and(|settings| {
        settings.exposure_mode == MoonlyExposureMode::LuminanceAdaptation
            && !settings.native_post
            && (settings.bloom_intensity - MoonlyPostSettings::default().bloom_intensity).abs()
                < f32::EPSILON
    }));
    assert!(
        app.world()
            .resource::<MoonlyBevyCompatWarnings>()
            .unsupported_options
            .is_empty()
    );
}

#[test]
fn native_only_camera_features_keep_the_core_graph() {
    let mut app = compat_app();
    app.register_required_components_with::<Camera3d, CameraRenderGraph>(|| {
        CameraRenderGraph::new(bevy::core_pipeline::schedule::Core3d)
    });
    let camera = app
        .world_mut()
        .spawn((Camera3d::default(), VolumetricFog::default()))
        .id();
    app.update();

    assert!(app.world().get::<MoonlyCamera>(camera).is_none());
    assert!(app.world().get::<MoonlyPostSettings>(camera).is_none());
    let graph = app.world().get::<CameraRenderGraph>(camera);
    assert!(graph.is_some_and(
        |graph| graph.0 == CameraRenderGraph::new(bevy::core_pipeline::schedule::Core3d).0
    ));
}

#[test]
fn native_camera_features_restore_and_reinstall_managed_cameras() {
    let mut app = compat_app();
    let camera = app.world_mut().spawn(Camera3d::default()).id();
    app.update();
    assert!(app.world().get::<MoonlyCamera>(camera).is_some());

    app.world_mut()
        .entity_mut(camera)
        .insert(VolumetricFog::default());
    app.update();
    assert!(app.world().get::<MoonlyCamera>(camera).is_none());
    assert!(app.world().get::<MoonlyPostSettings>(camera).is_none());
    assert!(app.world().get::<NoIndirectDrawing>(camera).is_none());

    app.world_mut().entity_mut(camera).remove::<VolumetricFog>();
    app.update();
    assert!(app.world().get::<MoonlyCamera>(camera).is_some());
    assert!(app.world().get::<MoonlyPostSettings>(camera).is_some());
}

#[test]
fn native_tonemapping_does_not_report_a_fallback() {
    let mut app = compat_app();
    app.world_mut()
        .spawn((Camera3d::default(), Tonemapping::TonyMcMapface));
    app.world_mut()
        .spawn((Camera3d::default(), Tonemapping::TonyMcMapface));
    app.update();
    app.update();

    let warnings = app.world().resource::<MoonlyBevyCompatWarnings>();
    assert!(warnings.unsupported_options.is_empty());
}

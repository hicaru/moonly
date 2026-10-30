use bevy::{
    camera::{Exposure, Hdr},
    core_pipeline::{prepass::DeferredPrepass, schedule::Core3d},
    prelude::*,
    render::{camera::CameraRenderGraph, view::NoIndirectDrawing},
};

use crate::{MoonlyCamera, MoonlyGpuDiagnostics, MoonlyRender, MoonlyRenderPlugin};

#[test]
fn plugin_builds_without_a_render_app() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MoonlyRenderPlugin));
    let camera = app.world_mut().spawn(MoonlyCamera::default()).id();
    app.update();
    assert!(app.world().contains_resource::<MoonlyGpuDiagnostics>());
    assert!(app.world().get::<Exposure>(camera).is_some());
}

#[test]
fn world_and_overlay_use_moonly_graph() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MoonlyRenderPlugin));
    let world = app.world_mut().spawn(MoonlyCamera::default()).id();
    let overlay = app
        .world_mut()
        .spawn(MoonlyCamera {
            role: crate::MoonlyCameraRole::Overlay,
        })
        .id();
    app.update();
    for camera in [world, overlay] {
        assert!(
            app.world()
                .get::<CameraRenderGraph>(camera)
                .is_some_and(|graph| graph.0 == CameraRenderGraph::new(MoonlyRender).0)
        );
        assert!(app.world().get::<DeferredPrepass>(camera).is_some());
    }
}

#[test]
fn native_forward_selects_core_graph_and_indirect_rendering() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MoonlyRenderPlugin));
    app.insert_resource(moonly_core::MoonlyNativeForward(true));
    let camera = app.world_mut().spawn(MoonlyCamera::default()).id();
    app.update();
    assert!(
        app.world()
            .get::<CameraRenderGraph>(camera)
            .is_some_and(|graph| graph.0 == CameraRenderGraph::new(Core3d).0)
    );
    assert!(app.world().get::<NoIndirectDrawing>(camera).is_none());
    assert!(app.world().get::<DeferredPrepass>(camera).is_none());
    assert!(app.world().get::<Hdr>(camera).is_none());
    let hdr_camera = app.world_mut().spawn((MoonlyCamera::default(), Hdr)).id();
    app.update();
    assert!(app.world().get::<Hdr>(hdr_camera).is_some());
    let deferred = app
        .world_mut()
        .spawn((MoonlyCamera::default(), DeferredPrepass))
        .id();
    app.update();
    assert!(app.world().get::<DeferredPrepass>(deferred).is_some());
    let preserved = app
        .world_mut()
        .spawn((
            MoonlyCamera::default(),
            Msaa::Sample4,
            NoIndirectDrawing,
            bevy::pbr::ScreenSpaceTransmission::default(),
        ))
        .id();
    app.update();
    assert!(matches!(
        app.world().get::<Msaa>(preserved),
        Some(Msaa::Sample4)
    ));
    assert!(app.world().get::<NoIndirectDrawing>(preserved).is_some());
    assert!(
        app.world()
            .get::<bevy::pbr::ScreenSpaceTransmission>(preserved)
            .is_some()
    );
    app.world_mut().entity_mut(camera).remove::<MoonlyCamera>();
    app.update();
    assert!(
        app.world()
            .get::<CameraRenderGraph>(camera)
            .is_some_and(|graph| graph.0 == CameraRenderGraph::new(Core3d).0)
    );
}

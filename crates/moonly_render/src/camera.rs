use bevy::{
    camera::{CameraMainTextureUsages, Exposure, Hdr},
    core_pipeline::{prepass::DeferredPrepass, schedule::Core3d, tonemapping::DebandDither},
    ecs::{lifecycle::HookContext, query::QueryItem, world::DeferredWorld},
    pbr::ScreenSpaceTransmission,
    prelude::*,
    render::{
        RenderApp, camera::CameraRenderGraph, extract_component::ExtractComponent,
        sync_component::SyncComponent, view::NoIndirectDrawing,
    },
};

use crate::graph::MoonlyRender;

/// Camera role in the Moonly render pipeline.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Reflect)]
pub enum MoonlyCameraRole {
    /// Main world view with geometry, lighting, and shadows.
    #[default]
    World,
    /// Secondary view that shares world exposure and post parameters.
    Overlay,
}

/// Opt-in marker for a camera that Moonly renders.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
#[component(on_insert = on_insert_moonly_camera, on_remove = on_remove_moonly_camera)]
#[require(
    Camera3d,
    Exposure,
    CameraRenderGraph = CameraRenderGraph::new(MoonlyRender),
    MoonlyDebugView
)]
pub struct MoonlyCamera {
    /// Pipeline role for this camera.
    pub role: MoonlyCameraRole,
}

fn on_insert_moonly_camera(world: DeferredWorld, context: HookContext) {
    configure_moonly_camera(world, context.entity);
}

pub(crate) fn sync_moonly_camera_roles(world: &mut World) {
    let cameras: Box<[Entity]> = world
        .query_filtered::<Entity, Changed<MoonlyCamera>>()
        .iter(world)
        .collect();
    for entity in cameras {
        configure_moonly_camera(world.into(), entity);
    }
}

fn configure_moonly_camera(mut world: DeferredWorld, entity: Entity) {
    let forward = world
        .get_resource::<moonly_core::MoonlyNativeForward>()
        .is_some_and(|settings| settings.0);
    let native = world
        .get_resource::<moonly_core::MoonlyNativePost>()
        .is_some_and(|settings| settings.0);
    let deferred = world.get::<DeferredPrepass>(entity).is_some();
    let usages = world
        .get::<CameraMainTextureUsages>(entity)
        .copied()
        .unwrap_or_default()
        .with(bevy::render::render_resource::TextureUsages::COPY_DST);
    let mut commands = world.commands();
    let mut entity = commands.entity(entity);
    if forward {
        entity.insert(CameraRenderGraph::new(Core3d));
        if !deferred {
            entity.remove::<DeferredPrepass>();
        }
        return;
    }
    entity.insert((
        CameraRenderGraph::new(MoonlyRender),
        Msaa::Off,
        NoIndirectDrawing,
        ScreenSpaceTransmission::default(),
        Hdr,
        DeferredPrepass,
        usages,
    ));
    if !native {
        entity.insert(DebandDither::Disabled);
    }
}

fn on_remove_moonly_camera(mut world: DeferredWorld, context: HookContext) {
    if let Ok(mut entity) = world.commands().get_entity(context.entity) {
        entity.insert(CameraRenderGraph::new(Core3d));
    }
}

impl SyncComponent<RenderApp> for MoonlyCamera {
    type Target = Self;
}

impl ExtractComponent<RenderApp> for MoonlyCamera {
    type QueryData = &'static Self;
    type QueryFilter = With<Camera>;
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(*item)
    }
}

/// Debug output for a Moonly camera.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum MoonlyDebugView {
    /// Final combined scene color.
    #[default]
    Final,
    /// World-space position or reconstructed depth data.
    Position,
    /// World-space normals.
    Normal,
    /// Base color and gloss data.
    Albedo,
    /// Hardware depth.
    Depth,
    /// HDR light accumulator.
    Accumulator,
    /// Shadow atlas depth.
    ShadowAtlas,
    /// Screen-space ambient occlusion.
    AmbientOcclusion,
    /// Volumetric lighting.
    Volumetrics,
    /// Local shadow transmittance atlas.
    TransmittanceAtlas,
}

impl MoonlyDebugView {
    #[must_use]
    pub const fn index(self) -> u32 {
        match self {
            Self::Final => 0,
            Self::Position => 1,
            Self::Normal => 2,
            Self::Albedo => 3,
            Self::Depth => 4,
            Self::Accumulator => 5,
            Self::ShadowAtlas => 6,
            Self::AmbientOcclusion => 7,
            Self::Volumetrics => 8,
            Self::TransmittanceAtlas => 9,
        }
    }
}

impl SyncComponent<RenderApp> for MoonlyDebugView {
    type Target = Self;
}

impl ExtractComponent<RenderApp> for MoonlyDebugView {
    type QueryData = &'static Self;
    type QueryFilter = With<MoonlyCamera>;
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(*item)
    }
}

/// Bypass exposure, bloom, optics, and tone mapping for linear mirror or capture output.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct MoonlyLinearOutput;

impl SyncComponent<RenderApp> for MoonlyLinearOutput {
    type Target = Self;
}

impl ExtractComponent<RenderApp> for MoonlyLinearOutput {
    type QueryData = &'static Self;
    type QueryFilter = With<MoonlyCamera>;
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(*item)
    }
}

/// Disable built-in Moonly volumetrics when another fog plugin owns the camera.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct MoonlyExternalFog;

impl SyncComponent<RenderApp> for MoonlyExternalFog {
    type Target = Self;
}

impl ExtractComponent<RenderApp> for MoonlyExternalFog {
    type QueryData = &'static Self;
    type QueryFilter = With<MoonlyCamera>;
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(*item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_changes_keep_moonly_render_graph() {
        let mut world = World::new();
        let entity = world
            .spawn((
                MoonlyCamera::default(),
                bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface,
            ))
            .id();
        for role in [MoonlyCameraRole::Overlay, MoonlyCameraRole::World] {
            if let Some(mut camera) = world.get_mut::<MoonlyCamera>(entity) {
                camera.role = role;
            }
            sync_moonly_camera_roles(&mut world);
            world.flush();
            assert!(
                world
                    .get::<CameraRenderGraph>(entity)
                    .is_some_and(|graph| graph.0 == CameraRenderGraph::new(MoonlyRender).0)
            );
            assert!(world.get::<DeferredPrepass>(entity).is_some());
            assert!(world.get_entity(entity).is_ok());
        }
        assert_eq!(
            world.get::<bevy::core_pipeline::tonemapping::Tonemapping>(entity),
            Some(&bevy::core_pipeline::tonemapping::Tonemapping::TonyMcMapface)
        );
    }
}

//! Automatic compatibility between native Bevy cameras and Moonly rendering.

mod capture;
mod post;
mod surfaces;
#[cfg(test)]
mod tests;

use bevy::{
    anti_alias::taa::TemporalAntiAliasing,
    core_pipeline::tonemapping::Tonemapping,
    light::{EnvironmentMapLight, Skybox, VolumetricFog},
    prelude::*,
    render::view::NoIndirectDrawing,
};
use moonly_core::{MoonlyNativeAmbient, MoonlyNativeForward, MoonlyNativePost};
use moonly_post::MoonlyExposureMode;
use moonly_render::{MoonlyCamera, MoonlyDebugView, MoonlyLinearOutput, MoonlyPostSettings};

pub use post::MoonlyBevyCompatWarnings;
use post::{ManagedMoonlyPostSettings, NativePostState};

use crate::MoonlyPlugin;

/// Compatibility alias for [`MoonlyPlugin::enhanced`].
///
/// New applications can add [`MoonlyPlugin`] directly. Native component adapters are built in.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoonlyBevyCompatPlugin;

/// Prevent automatic Moonly camera setup on this `Camera3d` entity.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct MoonlyBevyCompatOptOut;

#[derive(Component)]
struct ManagedMoonlyCamera {
    no_indirect: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CameraSetupAction {
    Install,
    Preserve,
    Restore,
    Skip,
}

#[derive(Clone, Copy)]
struct CameraSetupState<'a> {
    action: CameraSetupAction,
    has_settings: bool,
    no_indirect: bool,
    managed: Option<&'a ManagedMoonlyCamera>,
    tonemapping: Option<Tonemapping>,
}

type CameraSetup = (
    Has<MoonlyBevyCompatOptOut>,
    Has<MoonlyCamera>,
    Has<MoonlyPostSettings>,
    Has<Skybox>,
    Has<EnvironmentMapLight>,
    Has<VolumetricFog>,
    Has<TemporalAntiAliasing>,
    Has<NoIndirectDrawing>,
    Option<&'static ManagedMoonlyCamera>,
    Option<&'static Tonemapping>,
);

impl Plugin for MoonlyBevyCompatPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MoonlyPlugin::enhanced());
    }
}

pub(crate) fn configure(app: &mut App) {
    if !app.world().contains_resource::<MoonlyNativeAmbient>() {
        app.insert_resource(MoonlyNativeAmbient(true));
    }
    app.init_resource::<MoonlyNativeForward>()
        .init_resource::<MoonlyNativePost>()
        .init_resource::<MoonlyBevyCompatWarnings>()
        .register_type::<MoonlyBevyCompatOptOut>()
        .register_type::<MoonlyBevyCompatWarnings>()
        .add_systems(
            PreUpdate,
            (backfill_cameras, post::sync_native_post).chain(),
        )
        .add_systems(Last, post::restore_native_tonemapping);
    capture::configure(app);
    surfaces::configure(app);
}

fn backfill_cameras(mut commands: Commands, cameras: Query<(Entity, CameraSetup), With<Camera3d>>) {
    for (
        entity,
        (
            opt_out,
            has_camera,
            has_settings,
            skybox,
            environment,
            fog,
            taa,
            no_indirect,
            managed,
            tonemapping,
        ),
    ) in &cameras
    {
        let unsupported = skybox || environment || fog || taa;
        let managed_camera = managed.is_some();
        let action = match (opt_out, has_camera, unsupported, managed_camera) {
            (true, true, _, true) | (_, true, true, true) => CameraSetupAction::Restore,
            (true, _, _, _) | (_, _, true, _) => CameraSetupAction::Skip,
            (_, true, false, _) => CameraSetupAction::Preserve,
            _ => CameraSetupAction::Install,
        };
        let state = CameraSetupState {
            action,
            has_settings,
            no_indirect,
            managed,
            tonemapping: tonemapping.copied(),
        };
        install_camera(&mut commands, entity, state);
    }
}

fn install_camera(commands: &mut Commands, entity: Entity, state: CameraSetupState) {
    if state.action == CameraSetupAction::Skip {
        return;
    }
    let mut entity = commands.entity(entity);
    if state.action == CameraSetupAction::Restore {
        restore_native_camera(&mut entity, state.managed);
        return;
    }
    let install = state.action == CameraSetupAction::Install;
    if install {
        entity.insert((
            MoonlyCamera::default(),
            ManagedMoonlyCamera {
                no_indirect: state.no_indirect,
            },
        ));
        if let Some(debug) = debug_view_override() {
            entity.insert((debug, MoonlyLinearOutput));
        }
        restore_tonemapping(&mut entity, state.tonemapping);
    }
    if install && !state.has_settings {
        entity.insert((
            managed_post_settings(),
            ManagedMoonlyPostSettings,
            NativePostState {
                tonemapping: state.tonemapping,
            },
        ));
    }
}

fn restore_native_camera(entity: &mut EntityCommands, managed: Option<&ManagedMoonlyCamera>) {
    let no_indirect = managed.is_some_and(|state| state.no_indirect);
    entity.remove::<(
        MoonlyCamera,
        MoonlyPostSettings,
        ManagedMoonlyCamera,
        ManagedMoonlyPostSettings,
        NativePostState,
        NoIndirectDrawing,
    )>();
    if no_indirect {
        entity.insert(NoIndirectDrawing);
    }
}

fn restore_tonemapping(entity: &mut EntityCommands, tonemapping: Option<Tonemapping>) {
    if let Some(tonemapping) = tonemapping {
        entity.insert(tonemapping);
    } else {
        entity.remove::<Tonemapping>();
    }
}

fn managed_post_settings() -> MoonlyPostSettings {
    MoonlyPostSettings {
        exposure_mode: MoonlyExposureMode::Camera,
        native_post: true,
        ..MoonlyPostSettings::default()
    }
}

fn debug_view_override() -> Option<MoonlyDebugView> {
    match std::env::var("MOONLY_DEBUG_VIEW").ok()?.as_str() {
        "scene" => Some(MoonlyDebugView::Final),
        "position" => Some(MoonlyDebugView::Position),
        "normal" => Some(MoonlyDebugView::Normal),
        "albedo" => Some(MoonlyDebugView::Albedo),
        "depth" => Some(MoonlyDebugView::Depth),
        "accumulator" => Some(MoonlyDebugView::Accumulator),
        "shadow" => Some(MoonlyDebugView::ShadowAtlas),
        "ao" => Some(MoonlyDebugView::AmbientOcclusion),
        "volumetrics" => Some(MoonlyDebugView::Volumetrics),
        "transmittance" => Some(MoonlyDebugView::TransmittanceAtlas),
        _ => None,
    }
}

//! Render foundation for Moonly cameras, targets, passes, and diagnostics.

mod accumulate;
mod ao;
mod bloom;
pub mod camera;
mod caster_visibility;
mod combine;
mod depth_copy;
pub mod diagnostics;
mod extraction;
mod fog_camera;
mod forward;
pub mod forward_layout;
pub mod forward_lighting;
mod forward_prepare;
mod froxel_pass;
mod gpu_timer;
mod graph;
mod jitter;
mod light_packages;
mod light_volume;
mod local_volumetrics;
mod luminance;
mod luminance_readback;
mod material_adapters;
pub mod material_lut;
mod mesh_bind_groups;
mod mesh_bounds;
mod mesh_pipeline;
mod minmax;
mod motion_vectors;
mod native_post;
mod occlusion;
mod occlusion_state;
pub mod phase;
mod point_volume;
mod post;
mod projector;
mod queue;
mod reflected_lights;
mod render_app;
mod render_passes;
mod settings;
mod shaders;
mod shadow;
mod shadow_bind_groups;
mod shadow_pipeline;
mod shadow_queue;
mod shadow_target_types;
mod shadow_targets;
mod spot_pipeline;
mod sun_accumulate;
mod sun_shadow;
mod surface;
mod transmission;
mod ui;
mod view_targets;
mod visibility;
mod volumetrics;

use bevy::{
    pbr::MeshPipeline,
    prelude::*,
    render::{
        RenderApp, RenderDebugFlags, extract_component::ExtractComponentPlugin,
        extract_resource::ExtractResourcePlugin, render_phase::BinnedRenderPhasePlugin,
        uniform::UniformComponentPlugin,
    },
};
use moonly_post::{Hdr10PostConfig, MoonlyPostPlugin, OpticsMode};
use moonly_volumetrics::{MoonlyFroxelConfig, MoonlyVolumetricsPlugin, SunShaftsConfig};

pub use camera::{
    MoonlyCamera, MoonlyCameraRole, MoonlyDebugView, MoonlyExternalFog, MoonlyLinearOutput,
};
pub use caster_visibility::MoonlyCasterVisibility;
pub use diagnostics::{GpuTiming, MoonlyGpuDiagnostics, MoonlyPass};
pub use fog_camera::MoonlyWeatherFog;
pub use forward_layout::{moonly_forward_layout, moonly_forward_layout_for};
pub use forward_lighting::{MoonlyForwardView, SetMoonlyForwardBindGroup};
pub use graph::{MoonlyRender, MoonlyRenderSystems};
pub use material_adapters::MoonlyMaterialAdapters;
pub use material_lut::{MoonlyMaterialLut, generate_moonly_material_lut};
pub use settings::MoonlyPostSettings;
pub use surface::MoonlySurface;

use phase::{MoonlyAlphaMask3d, MoonlyOpaque3d, MoonlyShadow3d};

/// Moonly render foundation plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoonlyRenderPlugin;

impl Plugin for MoonlyRenderPlugin {
    fn build(&self, app: &mut App) {
        if app
            .world()
            .get_resource::<moonly_core::MoonlyNativeForward>()
            .is_some_and(|settings| settings.0)
        {
            render_app::configure_native_forward(app);
            return;
        }
        app.add_plugins((MoonlyVolumetricsPlugin, MoonlyPostPlugin));
        let luminance_sender = luminance_readback::install_luminance_channel(app);
        let gpu_timing_sender = gpu_timer::install_gpu_timing_channel(app);
        app.register_type::<MoonlyCamera>()
            .register_type::<MoonlyCameraRole>()
            .register_type::<MoonlyDebugView>()
            .register_type::<MoonlyExternalFog>()
            .register_type::<MoonlyLinearOutput>()
            .register_type::<MoonlyWeatherFog>()
            .register_type::<MoonlyPostSettings>()
            .register_type::<MoonlySurface>()
            .init_resource::<MoonlyGpuDiagnostics>()
            .init_resource::<moonly_core::MoonlyNativeForward>()
            .init_resource::<MoonlyMaterialLut>()
            .init_resource::<MoonlyMaterialAdapters>()
            .init_resource::<MoonlyCasterVisibility>()
            .init_resource::<mesh_bounds::MoonlyMeshBounds>()
            .add_systems(
                Update,
                (
                    camera::sync_moonly_camera_roles,
                    mesh_bounds::collect_mesh_bounds,
                    surface::sync_moonly_surfaces,
                    fog_camera::sync_fog_cameras,
                ),
            )
            .add_plugins(ExtractResourcePlugin::<mesh_bounds::MoonlyMeshBounds>::default())
            .add_plugins(ExtractResourcePlugin::<MoonlyMaterialAdapters>::default())
            .add_plugins(ExtractResourcePlugin::<MoonlyCasterVisibility>::default())
            .add_plugins(ExtractResourcePlugin::<Hdr10PostConfig>::default())
            .add_plugins(ExtractResourcePlugin::<moonly_post::LuminanceAdaptationState>::default())
            .add_plugins(ExtractResourcePlugin::<OpticsMode>::default())
            .add_plugins(ExtractResourcePlugin::<SunShaftsConfig>::default())
            .add_plugins(ExtractResourcePlugin::<MoonlyFroxelConfig>::default())
            .add_plugins(UniformComponentPlugin::<surface::MoonlySurfaceUniform>::default())
            .add_plugins(ExtractComponentPlugin::<surface::MoonlySurfaceUniform>::default())
            .add_plugins(ExtractComponentPlugin::<MoonlyCamera>::default())
            .add_plugins(ExtractComponentPlugin::<MoonlyDebugView>::default())
            .add_plugins(ExtractComponentPlugin::<MoonlyExternalFog>::default())
            .add_plugins(ExtractComponentPlugin::<MoonlyLinearOutput>::default())
            .add_plugins(ExtractComponentPlugin::<MoonlyPostSettings>::default())
            .add_plugins(ExtractComponentPlugin::<settings::MoonlyExposure>::default())
            .add_plugins(
                BinnedRenderPhasePlugin::<MoonlyOpaque3d, MeshPipeline>::new(
                    RenderDebugFlags::default(),
                ),
            )
            .add_plugins(
                BinnedRenderPhasePlugin::<MoonlyAlphaMask3d, MeshPipeline>::new(
                    RenderDebugFlags::default(),
                ),
            )
            .add_plugins(
                BinnedRenderPhasePlugin::<MoonlyShadow3d, MeshPipeline>::new(
                    RenderDebugFlags::default(),
                ),
            );
        render_app::configure_shader_assets(app);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.insert_resource(luminance_sender);
        render_app.insert_resource(gpu_timing_sender);
        render_app::configure_render_app(render_app);
    }
}

#[cfg(test)]
mod lib_tests;

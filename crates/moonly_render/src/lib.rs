//! Render foundation for Moonly cameras, targets, passes, and diagnostics.

mod accumulate;
mod ao;
mod bloom;
pub mod camera;
mod caster_visibility;
mod combine;
pub mod diagnostics;
mod extraction;
mod gpu_timer;
mod graph;
mod jitter;
mod light_volume;
mod local_volumetrics;
mod luminance;
mod luminance_readback;
pub mod material_lut;
mod mesh_bind_groups;
mod mesh_bounds;
mod mesh_pipeline;
mod minmax;
mod occlusion;
mod occlusion_state;
pub mod phase;
mod point_volume;
mod post;
mod projector;
mod queue;
mod reflected_lights;
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
mod view_targets;
mod visibility;
mod volumetrics;

use bevy::{
    app::SubApp,
    asset::{embedded_asset, io::embedded::EmbeddedAssetRegistry},
    pbr::{MeshPipeline, allocate_uniforms, early_gpu_preprocess, unpack_bins},
    prelude::*,
    render::{
        ExtractSchedule, GpuResourceAppExt, Render, RenderApp, RenderDebugFlags, RenderStartup,
        RenderSystems,
        extract_component::ExtractComponentPlugin,
        extract_resource::ExtractResourcePlugin,
        render_phase::{AddRenderCommand, BinnedRenderPhasePlugin, DrawFunctions},
        render_resource::{SpecializedMeshPipelines, SpecializedRenderPipelines},
        uniform::UniformComponentPlugin,
    },
};
use moonly_post::{Hdr10PostConfig, LuminanceAdaptationState, MoonlyPostPlugin, OpticsMode};
use moonly_volumetrics::{MoonlyVolumetricsPlugin, SunShaftsConfig};

pub use camera::{MoonlyCamera, MoonlyCameraRole, MoonlyDebugView, MoonlyLinearOutput};
pub use caster_visibility::MoonlyCasterVisibility;
pub use diagnostics::{GpuTiming, MoonlyGpuDiagnostics, MoonlyPass};
pub use graph::MoonlyRender;
pub use material_lut::{MoonlyMaterialLut, generate_moonly_material_lut};
pub use surface::MoonlySurface;

use combine::MoonlyCombinePipeline;
use mesh_bind_groups::{DrawMoonlyMesh, MoonlyMeshBindGroups};
use mesh_pipeline::MoonlyMeshPipeline;
use phase::{MoonlyAlphaMask3d, MoonlyOpaque3d, MoonlyShadow3d};
use queue::MoonlyQueuedEntities;
use shadow_bind_groups::MoonlyShadowMeshBindGroups;
use shadow_pipeline::MoonlyShadowMeshPipeline;
use shadow_queue::{DrawMoonlyShadow, MoonlyShadowQueuedEntities};

/// Moonly render foundation plugin.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoonlyRenderPlugin;

impl Plugin for MoonlyRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((MoonlyVolumetricsPlugin, MoonlyPostPlugin));
        let luminance_sender = luminance_readback::install_luminance_channel(app);
        let gpu_timing_sender = gpu_timer::install_gpu_timing_channel(app);
        app.register_type::<MoonlyCamera>()
            .register_type::<MoonlyCameraRole>()
            .register_type::<MoonlyDebugView>()
            .register_type::<MoonlyLinearOutput>()
            .register_type::<MoonlySurface>()
            .init_resource::<MoonlyGpuDiagnostics>()
            .init_resource::<MoonlyMaterialLut>()
            .init_resource::<MoonlyCasterVisibility>()
            .init_resource::<mesh_bounds::MoonlyMeshBounds>()
            .add_systems(
                Update,
                (
                    mesh_bounds::collect_mesh_bounds,
                    surface::sync_moonly_surfaces,
                ),
            )
            .add_plugins(ExtractResourcePlugin::<mesh_bounds::MoonlyMeshBounds>::default())
            .add_plugins(ExtractResourcePlugin::<MoonlyCasterVisibility>::default())
            .add_plugins(ExtractResourcePlugin::<Hdr10PostConfig>::default())
            .add_plugins(ExtractResourcePlugin::<LuminanceAdaptationState>::default())
            .add_plugins(ExtractResourcePlugin::<OpticsMode>::default())
            .add_plugins(ExtractResourcePlugin::<SunShaftsConfig>::default())
            .add_plugins(UniformComponentPlugin::<surface::MoonlySurfaceUniform>::default())
            .add_plugins(ExtractComponentPlugin::<surface::MoonlySurfaceUniform>::default())
            .add_plugins(ExtractComponentPlugin::<MoonlyCamera>::default())
            .add_plugins(ExtractComponentPlugin::<MoonlyDebugView>::default())
            .add_plugins(ExtractComponentPlugin::<MoonlyLinearOutput>::default())
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
        if app.world().contains_resource::<EmbeddedAssetRegistry>() {
            embedded_asset!(app, "gbuffer.wesl");
            embedded_asset!(app, "ao.wesl");
            embedded_asset!(app, "bloom.wesl");
            embedded_asset!(app, "combine.wesl");
            embedded_asset!(app, "post.wesl");
            embedded_asset!(app, "luminance.wesl");
            embedded_asset!(app, "volumetrics.wesl");
            embedded_asset!(app, "accum_spot.wesl");
            embedded_asset!(app, "accum_direct.wesl");
            embedded_asset!(app, "minmax.wesl");
            embedded_asset!(app, "shadow_depth.wesl");
        }
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.insert_resource(luminance_sender);
        render_app.insert_resource(gpu_timing_sender);
        configure_render_app(render_app);
    }
}

fn configure_render_app(render_app: &mut SubApp) {
    render_app
        .add_schedule(graph::base_schedule())
        .add_systems(
            MoonlyRender,
            (
                allocate_uniforms,
                unpack_bins,
                early_gpu_preprocess,
                shadow::render_spot_shadow,
                sun_shadow::render_sun_shadow,
                minmax::render_sun_minmax,
                graph::render_gbuffer,
                ao::render_ao,
                occlusion::render_spot_accumulation,
                local_volumetrics::render_local_volumetrics,
                sun_accumulate::render_sun_accumulation,
                volumetrics::render_volumetrics,
                combine::render_combine,
                luminance::render_luminance,
                bloom::render_bloom,
                post::render_post,
                gpu_timer::resolve_gpu_timings,
            )
                .chain(),
        )
        .add_systems(
            ExtractSchedule,
            (
                extraction::extract_moonly_camera_phases,
                extraction::extract_moonly_shadow_phases,
            ),
        )
        .add_systems(
            RenderStartup,
            (
                shaders::init_gbuffer_shader,
                gpu_timer::init_gpu_timer,
                ao::init_ao_pipeline,
                bloom::init_bloom_pipeline,
                combine::init_combine_pipeline,
                post::init_post_pipeline,
                luminance::init_luminance_pipeline,
                volumetrics::init_volumetric_pipeline,
                material_lut::init_moonly_material_lut_texture,
                surface::init_surface_layout,
                jitter::init_jitter_texture,
                projector::init_fallback_projector,
                light_volume::init_light_volume,
                spot_pipeline::init_spot_pipeline,
                sun_accumulate::init_sun_pipeline,
                minmax::init_minmax_pipeline,
                shadow_pipeline::init_shadow_pipeline.after(bevy::pbr::init_prepass_pipeline),
            ),
        );
    configure_prepare_systems(render_app);
    configure_draw_commands(render_app);
}

fn configure_prepare_systems(render_app: &mut SubApp) {
    render_app.add_systems(
        Render,
        (
            (
                view_targets::prepare_view_targets,
                luminance::prepare_luminance_targets,
                shadow_targets::prepare_spot_shadow_targets,
                minmax::prepare_minmax_bind_groups,
                ao::prepare_ao_bind_groups,
                accumulate::prepare_spot_bind_groups,
                occlusion::prepare_occlusion,
                sun_accumulate::prepare_sun_bind_groups,
                volumetrics::prepare_volumetric_bind_groups,
                combine::prepare_combine_bind_groups,
                combine::prepare_combine_pipelines,
                post::prepare_post_bind_groups,
                post::prepare_post_pipelines,
                luminance::prepare_luminance_bind_groups,
                bloom::prepare_bloom_bind_groups,
            )
                .chain()
                .in_set(RenderSystems::PrepareResources),
            (
                queue::queue_moonly_meshes,
                shadow_queue::queue_shadow_meshes,
            )
                .in_set(RenderSystems::QueueMeshes),
            (
                mesh_bind_groups::prepare_moonly_mesh_bind_groups,
                shadow_bind_groups::prepare_shadow_mesh_bind_groups,
                surface::prepare_surface_bind_group,
            )
                .in_set(RenderSystems::PrepareBindGroups),
        ),
    );
}

fn configure_draw_commands(render_app: &mut SubApp) {
    render_app
        .init_resource::<DrawFunctions<MoonlyOpaque3d>>()
        .init_resource::<DrawFunctions<MoonlyAlphaMask3d>>()
        .init_resource::<DrawFunctions<MoonlyShadow3d>>()
        .add_render_command::<MoonlyOpaque3d, DrawMoonlyMesh>()
        .add_render_command::<MoonlyAlphaMask3d, DrawMoonlyMesh>()
        .add_render_command::<MoonlyShadow3d, DrawMoonlyShadow>()
        .init_resource::<MoonlyMeshBindGroups>()
        .init_resource::<MoonlyShadowMeshBindGroups>()
        .init_resource::<MoonlyQueuedEntities>()
        .init_resource::<MoonlyShadowQueuedEntities>()
        .init_gpu_resource::<SpecializedMeshPipelines<MoonlyMeshPipeline>>()
        .init_gpu_resource::<SpecializedMeshPipelines<MoonlyShadowMeshPipeline>>()
        .init_gpu_resource::<SpecializedRenderPipelines<MoonlyCombinePipeline>>()
        .init_gpu_resource::<SpecializedRenderPipelines<post::MoonlyPostPipeline>>()
        .allow_ambiguous_resource::<SpecializedRenderPipelines<MoonlyCombinePipeline>>()
        .allow_ambiguous_resource::<SpecializedRenderPipelines<post::MoonlyPostPipeline>>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_builds_without_a_render_app() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, MoonlyRenderPlugin));
        app.world_mut().spawn(MoonlyCamera::default());
        app.update();
        assert!(app.world().contains_resource::<MoonlyGpuDiagnostics>());
    }
}

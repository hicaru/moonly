use bevy::{
    app::SubApp,
    asset::{embedded_asset, io::embedded::EmbeddedAssetRegistry},
    prelude::*,
    render::{
        ExtractSchedule, GpuResourceAppExt, Render, RenderStartup, RenderSystems,
        render_phase::{AddRenderCommand, DrawFunctions},
        render_resource::{SpecializedMeshPipelines, SpecializedRenderPipelines},
    },
    shader::load_shader_library,
};

use crate::phase::{MoonlyAlphaMask3d, MoonlyOpaque3d, MoonlyShadow3d};
use crate::{
    MoonlyRender, accumulate, ao, bloom, camera, caster_visibility, combine, depth_copy,
    diagnostics, extraction, froxel_pass, gpu_timer, graph, jitter, light_packages, light_volume,
    luminance, material_adapters, material_lut, mesh_bind_groups,
    mesh_pipeline::MoonlyMeshPipeline, minmax, occlusion, post, projector, queue, render_passes,
    settings, shaders, shadow_bind_groups, shadow_pipeline, shadow_queue, shadow_targets,
    spot_pipeline, sun_accumulate, surface, transmission, view_targets, volumetrics,
};

pub(crate) fn configure_shader_assets(app: &mut App) {
    if !app.world().contains_resource::<EmbeddedAssetRegistry>() {
        return;
    }
    load_shader_library!(app, "lighting.wesl");
    load_shader_library!(app, "gbuffer_surface.wesl");
    embedded_asset!(app, "gbuffer.wesl");
    embedded_asset!(app, "ao.wesl");
    embedded_asset!(app, "bloom.wesl");
    embedded_asset!(app, "combine.wesl");
    embedded_asset!(app, "depth_copy.wesl");
    embedded_asset!(app, "post.wesl");
    embedded_asset!(app, "luminance.wesl");
    embedded_asset!(app, "volumetrics.wesl");
    embedded_asset!(app, "froxel.wesl");
    occlusion::register_assets(app);
    embedded_asset!(app, "accum_direct.wesl");
    embedded_asset!(app, "minmax.wesl");
    embedded_asset!(app, "shadow_depth.wesl");
}

pub(crate) fn configure_native_forward(app: &mut App) {
    app.register_type::<camera::MoonlyCamera>()
        .register_type::<camera::MoonlyCameraRole>()
        .register_type::<camera::MoonlyDebugView>()
        .register_type::<camera::MoonlyExternalFog>()
        .register_type::<camera::MoonlyLinearOutput>()
        .register_type::<settings::MoonlyPostSettings>()
        .register_type::<surface::MoonlySurface>()
        .init_resource::<diagnostics::MoonlyGpuDiagnostics>()
        .init_resource::<material_adapters::MoonlyMaterialAdapters>()
        .init_resource::<caster_visibility::MoonlyCasterVisibility>();
}

pub(crate) fn configure_render_app(render_app: &mut SubApp) {
    render_app
        .add_schedule(graph::base_schedule())
        .add_systems(MoonlyRender, render_passes::render_systems())
        .add_systems(
            Render,
            settings::configure_view_exposure
                .before(bevy::render::view::prepare_view_uniforms)
                .run_if(settings::native_forward_disabled),
        )
        .add_systems(
            ExtractSchedule,
            (
                extraction::extract_moonly_camera_phases,
                extraction::extract_moonly_shadow_phases,
            )
                .run_if(settings::native_forward_disabled),
        )
        .add_systems(
            RenderStartup,
            (
                shaders::init_gbuffer_shader,
                gpu_timer::init_gpu_timer,
                ao::init_ao_pipeline,
                bloom::init_bloom_pipeline,
                combine::init_combine_pipeline,
                depth_copy::init_depth_copy_pipeline,
                froxel_pass::init_froxel_texture,
                froxel_pass::init_froxel_pipeline,
                post::init_post_pipeline,
                luminance::init_luminance_pipeline,
                volumetrics::init_volumetric_pipeline,
                surface::init_surface_layout,
                material_lut::init_moonly_material_lut_texture,
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
                light_packages::prepare_view_light_packages,
                view_targets::prepare_view_targets
                    .after(bevy::core_pipeline::core_3d::prepare_core_3d_depth_textures),
                depth_copy::prepare_depth_copy_bind_groups,
                transmission::prepare_transmission_textures,
                luminance::prepare_luminance_targets,
                shadow_targets::prepare_spot_shadow_targets,
                minmax::prepare_minmax_bind_groups,
                ao::prepare_ao_bind_groups,
                accumulate::prepare_spot_bind_groups,
                occlusion::prepare_occlusion,
                sun_accumulate::prepare_sun_bind_groups,
                volumetrics::prepare_volumetric_bind_groups,
                froxel_pass::prepare_froxel_bind_groups,
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
        )
            .run_if(settings::native_forward_disabled),
    );
}

fn configure_draw_commands(render_app: &mut SubApp) {
    render_app
        .init_resource::<DrawFunctions<MoonlyOpaque3d>>()
        .init_resource::<DrawFunctions<MoonlyAlphaMask3d>>()
        .init_resource::<DrawFunctions<MoonlyShadow3d>>()
        .add_render_command::<MoonlyOpaque3d, mesh_bind_groups::DrawMoonlyMesh>()
        .add_render_command::<MoonlyAlphaMask3d, mesh_bind_groups::DrawMoonlyMesh>()
        .add_render_command::<MoonlyShadow3d, shadow_queue::DrawMoonlyShadow>()
        .init_resource::<mesh_bind_groups::MoonlyMeshBindGroups>()
        .init_resource::<shadow_bind_groups::MoonlyShadowMeshBindGroups>()
        .init_resource::<queue::MoonlyQueuedEntities>()
        .init_resource::<shadow_queue::MoonlyShadowQueuedEntities>()
        .init_gpu_resource::<SpecializedMeshPipelines<MoonlyMeshPipeline>>()
        .init_gpu_resource::<SpecializedMeshPipelines<shadow_pipeline::MoonlyShadowMeshPipeline>>()
        .init_gpu_resource::<SpecializedRenderPipelines<combine::MoonlyCombinePipeline>>()
        .init_gpu_resource::<SpecializedRenderPipelines<post::MoonlyPostPipeline>>()
        .allow_ambiguous_resource::<SpecializedRenderPipelines<combine::MoonlyCombinePipeline>>()
        .allow_ambiguous_resource::<SpecializedRenderPipelines<post::MoonlyPostPipeline>>();
}

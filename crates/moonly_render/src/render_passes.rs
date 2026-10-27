use bevy::{
    core_pipeline::{tonemapping::tonemapping, upscaling::upscaling},
    ecs::{
        schedule::{
            IntoScheduleConfigs,
            common_conditions::{not, resource_equals},
        },
        system::ScheduleSystem,
    },
    pbr::{
        allocate_uniforms, early_gpu_preprocess, late_gpu_preprocess, main_transmissive_pass_3d,
        unpack_bins,
    },
};

use crate::{
    ao, bloom, combine, depth_copy, forward, froxel_pass, gpu_timer,
    graph::{self, MoonlyRenderSystems},
    local_volumetrics, luminance, minmax, occlusion, post, shadow, sun_accumulate, sun_shadow,
    volumetrics,
};

pub(crate) fn render_systems() -> impl IntoScheduleConfigs<ScheduleSystem, ()> {
    (
        (
            allocate_uniforms,
            unpack_bins,
            early_gpu_preprocess,
            shadow::render_spot_shadow,
            sun_shadow::render_sun_shadow,
            minmax::render_sun_minmax,
            graph::render_gbuffer,
            late_gpu_preprocess,
            ao::render_ao,
            occlusion::render_spot_accumulation,
            occlusion::native::render_native_local,
            local_volumetrics::render_local_volumetrics,
            sun_accumulate::render_sun_accumulation,
            volumetrics::render_volumetrics,
            froxel_pass::render_froxel,
            combine::render_combine,
            depth_copy::render_depth_copy,
        )
            .chain()
            .in_set(MoonlyRenderSystems::DeferredPass),
        main_transmissive_pass_3d.in_set(MoonlyRenderSystems::TransmissivePass),
        forward::render_transparent.in_set(MoonlyRenderSystems::MainPass),
        (
            (
                luminance::render_luminance,
                bloom::render_bloom,
                post::render_post,
            )
                .chain()
                .run_if(not(resource_equals(moonly_core::MoonlyNativePost(true)))),
            tonemapping.run_if(resource_equals(moonly_core::MoonlyNativePost(true))),
            upscaling.run_if(resource_equals(moonly_core::MoonlyNativePost(true))),
            gpu_timer::resolve_gpu_timings,
        )
            .chain()
            .in_set(MoonlyRenderSystems::PostProcess),
    )
        .chain()
}

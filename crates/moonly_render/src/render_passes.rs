use bevy::{
    ecs::{schedule::IntoScheduleConfigs, system::ScheduleSystem},
    pbr::{allocate_uniforms, early_gpu_preprocess, late_gpu_preprocess, unpack_bins},
};

use crate::{
    ao, bloom, combine, depth_copy, forward, froxel_pass, gpu_timer,
    graph::{self, MoonlyRenderSystems},
    local_volumetrics, luminance, minmax, native_post, occlusion, post, shadow, sun_accumulate,
    sun_shadow, transmission, ui, volumetrics,
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
        transmission::render_transmissive.in_set(MoonlyRenderSystems::TransmissivePass),
        forward::render_transparent.in_set(MoonlyRenderSystems::MainPass),
        (
            (
                luminance::render_luminance,
                native_post::render_native_bloom_input,
                bevy::post_process::bloom::bloom,
                native_post::render_native_bloom_output,
                bloom::render_bloom,
                post::render_post,
                native_post::render_tonemapping,
                forward::render_opaque,
            )
                .chain(),
            gpu_timer::resolve_gpu_timings,
        )
            .chain()
            .in_set(MoonlyRenderSystems::PostProcess),
        ui::render_ui.in_set(MoonlyRenderSystems::Ui),
        native_post::render_upscaling.in_set(MoonlyRenderSystems::FinalOutput),
    )
        .chain()
}

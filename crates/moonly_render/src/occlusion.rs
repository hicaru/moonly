use bevy::{
    asset::embedded_asset,
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_resource::{IndexFormat, PipelineCache, RenderPassDescriptor, StoreOp},
        renderer::{RenderContext, RenderDevice, ViewQuery},
    },
};

use crate::{
    accumulate::{LocalVolumeKind, MoonlySpotBindGroups},
    camera::MoonlyCamera,
    light_volume::{CONE_INDEX_COUNT, MoonlyLightVolume},
    occlusion_state::MoonlyOcclusionState,
    point_volume::POINT_INDEX_COUNT,
    spot_pipeline::MoonlySpotPipeline,
    view_targets::MoonlyViewTargets,
};

pub(crate) mod native;

pub(crate) fn register_assets(app: &mut App) {
    embedded_asset!(app, "accum_spot.wesl");
    embedded_asset!(app, "accum_native.wesl");
}

pub(crate) fn prepare_occlusion(
    mut commands: Commands,
    device: Res<RenderDevice>,
    mut views: Query<
        (
            Entity,
            &MoonlySpotBindGroups,
            Option<&mut MoonlyOcclusionState>,
        ),
        With<MoonlyCamera>,
    >,
) {
    for (entity, groups, state) in &mut views {
        if let Some(mut state) = state {
            state.resize_if_needed(&device, groups.0.len());
            continue;
        }
        commands
            .entity(entity)
            .insert(MoonlyOcclusionState::new(&device, groups.0.len()));
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct SpotAccumRenderInputs<'w> {
    pipeline_cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlySpotPipeline>,
    volume: Res<'w, MoonlyLightVolume>,
    timer: Res<'w, crate::gpu_timer::MoonlyGpuTimer>,
}

pub(crate) fn render_spot_accumulation(
    view: ViewQuery<
        (
            &MoonlyViewTargets,
            &MoonlySpotBindGroups,
            &MoonlyOcclusionState,
        ),
        With<MoonlyCamera>,
    >,
    inputs: SpotAccumRenderInputs,
    mut context: RenderContext,
) {
    let SpotAccumRenderInputs {
        pipeline_cache,
        pipeline,
        volume,
        timer,
    } = inputs;
    let (targets, bind_groups, occlusion) = view.into_inner();
    if bind_groups.0.is_empty() {
        return;
    }
    let (Some(back_mask), Some(front_mask), Some(shade)) = (
        pipeline_cache.get_render_pipeline(pipeline.back_mask),
        pipeline_cache.get_render_pipeline(pipeline.front_mask),
        pipeline_cache.get_render_pipeline(pipeline.shade),
    ) else {
        return;
    };
    occlusion.collect_results();
    occlusion.update_keys(bind_groups.0.iter().map(|binding| binding.key));
    let attachments = [Some(targets.accumulator.get_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_accum_spot"),
        color_attachments: &attachments,
        depth_stencil_attachment: Some(targets.depth.get_attachment(StoreOp::Store)),
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::AccumLocal),
        occlusion_query_set: Some(&occlusion.query_set),
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_accum_spot");
    for (query_index, binding) in bind_groups.0.iter().enumerate() {
        let index_count = match binding.kind {
            LocalVolumeKind::Cone => {
                pass.set_vertex_buffer(0, volume.vertices.slice(..));
                pass.set_index_buffer(volume.indices.slice(..), IndexFormat::Uint16);
                CONE_INDEX_COUNT
            }
            LocalVolumeKind::Sphere => {
                pass.set_vertex_buffer(0, volume.point.vertices.slice(..));
                pass.set_index_buffer(volume.point.indices.slice(..), IndexFormat::Uint16);
                POINT_INDEX_COUNT
            }
        };
        pass.set_bind_group(0, &binding.bind_group, &[]);
        pass.wgpu_pass().begin_occlusion_query(query_index as u32);
        for (render_pipeline, stencil_reference) in [(back_mask, 5), (front_mask, 1)] {
            pass.set_render_pipeline(render_pipeline);
            pass.set_stencil_reference(stencil_reference);
            pass.draw_indexed(0..index_count, 0, 0..1);
        }
        // Keep shading outside the query to avoid feedback from the previous visibility result.
        pass.wgpu_pass().end_occlusion_query();
        if occlusion.is_visible(binding.key) {
            pass.set_render_pipeline(shade);
            pass.set_stencil_reference(5);
            pass.draw_indexed(0..index_count, 0, 0..1);
        }
    }
    span.end(&mut pass);
    drop(pass);
    occlusion.resolve_and_schedule(context.command_encoder());
}

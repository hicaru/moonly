use bevy::{
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_resource::{IndexFormat, PipelineCache, RenderPassDescriptor},
        renderer::{RenderContext, ViewQuery},
    },
};

use crate::{
    accumulate::{LocalVolumeKind, MoonlySpotBindGroups},
    camera::MoonlyCamera,
    light_volume::{CONE_INDEX_COUNT, MoonlyLightVolume},
    point_volume::POINT_INDEX_COUNT,
    spot_pipeline::MoonlySpotPipeline,
    view_targets::MoonlyViewTargets,
};

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct LocalVolumeRenderInputs<'w> {
    pipeline_cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlySpotPipeline>,
    volume: Res<'w, MoonlyLightVolume>,
    timer: Res<'w, crate::gpu_timer::MoonlyGpuTimer>,
}

type LocalVolumeView<'w, 's> = ViewQuery<
    'w,
    's,
    (&'static MoonlyViewTargets, &'static MoonlySpotBindGroups),
    (With<MoonlyCamera>, Without<crate::MoonlyExternalFog>),
>;

pub(crate) fn render_local_volumetrics(
    view: LocalVolumeView,
    inputs: LocalVolumeRenderInputs,
    mut context: RenderContext,
) {
    let LocalVolumeRenderInputs {
        pipeline_cache,
        pipeline,
        volume,
        timer,
    } = inputs;
    let (targets, groups) = view.into_inner();
    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.volumetric) else {
        return;
    };
    let attachments = [Some(targets.local_volumetrics.get_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_local_volumetrics"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::Volumetrics),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_local_volumetrics");
    pass.set_render_pipeline(render_pipeline);
    for binding in groups.0.iter().filter(|binding| binding.volumetric) {
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
        pass.draw_indexed(0..index_count, 0, 0..1);
    }
    span.end(&mut pass);
}

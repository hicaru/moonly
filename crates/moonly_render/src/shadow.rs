use std::sync::atomic::Ordering;

use bevy::{
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{RenderPassDescriptor, StoreOp},
        renderer::{RenderContext, ViewQuery},
        view::ExtractedView,
    },
};

use crate::{
    camera::MoonlyCamera,
    phase::MoonlyShadow3d,
    shadow_targets::{MoonlyShadowViewIndex, MoonlySpotShadowTarget},
};

pub(crate) fn render_spot_shadow(
    world: &World,
    view: ViewQuery<
        (
            &ExtractedView,
            &MoonlySpotShadowTarget,
            &MoonlyShadowViewIndex,
        ),
        With<MoonlyCamera>,
    >,
    phases: Res<ViewBinnedRenderPhases<MoonlyShadow3d>>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    mut context: RenderContext,
) {
    let view_entity = view.entity();
    let (extracted_view, target, selected) = view.into_inner();
    let Some(phase) = phases.get(&extracted_view.retained_view_entity) else {
        return;
    };
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    for (batch_index, batch) in target.batches.iter().enumerate() {
        if batch.views.is_empty() {
            continue;
        }
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("moonly_spot_shadow"),
            color_attachments: &[],
            depth_stencil_attachment: Some(batch.depth.get_attachment(StoreOp::Store)),
            timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::ShadowLocal),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let span = diagnostics.pass_span(&mut pass, "moonly_shadow_spot");
        selected.batch.store(batch_index, Ordering::Relaxed);
        for (view_index, view) in batch.views.iter().enumerate() {
            selected.view.store(view_index, Ordering::Relaxed);
            let size = view.rect.size as f32;
            pass.set_viewport(view.rect.x as f32, view.rect.y as f32, size, size, 0.0, 1.0);
            if let Err(error) = phase.render(&mut pass, world, view_entity) {
                error!("Moonly spot shadow phase failed: {error:?}");
            }
        }
        span.end(&mut pass);
    }
}

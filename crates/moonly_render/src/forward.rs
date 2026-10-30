use bevy::{
    camera::{MainPassResolutionOverride, Viewport},
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    ecs::system::SystemParam,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::{RenderPassDescriptor, StoreOp},
        renderer::{RenderContext, ViewQuery},
        view::{ExtractedView, ViewDepthStencilTexture},
    },
};

use crate::{camera::MoonlyCamera, view_targets::MoonlyViewTargets};

type ForwardViewQuery<'w, 's> = ViewQuery<
    'w,
    's,
    (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static MoonlyViewTargets,
        Option<&'static ViewDepthStencilTexture>,
        Option<&'static MainPassResolutionOverride>,
    ),
    With<MoonlyCamera>,
>;

#[derive(SystemParam)]
pub(crate) struct ForwardInputs<'w> {
    timer: Res<'w, crate::gpu_timer::MoonlyGpuTimer>,
}

pub(crate) fn render_transparent(
    world: &World,
    view: ForwardViewQuery,
    phases: Res<ViewSortedRenderPhases<Transparent3d>>,
    inputs: ForwardInputs,
    mut context: RenderContext,
) {
    let view_entity = view.entity();
    let (camera, extracted_view, targets, depth, resolution_override) = view.into_inner();
    let Some(depth) = depth else {
        return;
    };
    let Some(phase) = phases.get(&extracted_view.retained_view_entity) else {
        return;
    };
    if phase.items.is_empty() {
        return;
    }
    let attachments = [Some(targets.scene.get_unsampled_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_transparent"),
        color_attachments: &attachments,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: inputs
            .timer
            .timestamp_writes(crate::diagnostics::MoonlyPass::Forward),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_transparent");
    if let Some(viewport) =
        Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override)
            .map(crate::view_targets::private_viewport)
    {
        pass.set_camera_viewport(&viewport);
    }
    if let Err(error) = phase.render(&mut pass, world, view_entity) {
        error!("Moonly transparent phase failed: {error:?}");
    }
    span.end(&mut pass);
}

pub(crate) fn render_opaque(
    world: &World,
    view: ForwardViewQuery,
    opaque_phases: Res<ViewBinnedRenderPhases<Opaque3d>>,
    alpha_phases: Res<ViewBinnedRenderPhases<AlphaMask3d>>,
    mut context: RenderContext,
) {
    let view_entity = view.entity();
    let (camera, extracted_view, targets, depth, resolution_override) = view.into_inner();
    let Some(depth) = depth else {
        return;
    };
    let (Some(opaque), Some(alpha)) = (
        opaque_phases.get(&extracted_view.retained_view_entity),
        alpha_phases.get(&extracted_view.retained_view_entity),
    ) else {
        return;
    };
    if opaque.is_empty() && alpha.is_empty() {
        return;
    }
    let attachments = [Some(targets.post_process.get_unsampled_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_forward_opaque"),
        color_attachments: &attachments,
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_forward_opaque");
    if let Some(viewport) =
        Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override)
            .map(crate::view_targets::private_viewport)
    {
        pass.set_camera_viewport(&viewport);
    }
    if let Err(error) = opaque.render(&mut pass, world, view_entity) {
        error!("Moonly opaque forward phase failed: {error:?}");
    }
    if let Err(error) = alpha.render(&mut pass, world, view_entity) {
        error!("Moonly alpha forward phase failed: {error:?}");
    }
    span.end(&mut pass);
}

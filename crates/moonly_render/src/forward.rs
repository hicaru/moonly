use bevy::{
    camera::{MainPassResolutionOverride, Viewport},
    core_pipeline::core_3d::Transparent3d,
    ecs::system::SystemParam,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_phase::ViewSortedRenderPhases,
        render_resource::{RenderPassDescriptor, StoreOp},
        renderer::{RenderContext, ViewQuery},
        view::{ExtractedView, ViewDepthStencilTexture, ViewTarget},
    },
};

use crate::camera::MoonlyCamera;

type ForwardViewQuery<'w, 's> = ViewQuery<
    'w,
    's,
    (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static ViewTarget,
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
    let (camera, extracted_view, target, depth, resolution_override) = view.into_inner();
    let Some(depth) = depth else {
        return;
    };
    let Some(phase) = phases.get(&extracted_view.retained_view_entity) else {
        return;
    };
    if phase.items.is_empty() {
        return;
    }
    let attachments = [Some(target.get_unsampled_color_attachment())];
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
    {
        pass.set_camera_viewport(&viewport);
    }
    if let Err(error) = phase.render(&mut pass, world, view_entity) {
        error!("Moonly transparent phase failed: {error:?}");
    }
    span.end(&mut pass);
}

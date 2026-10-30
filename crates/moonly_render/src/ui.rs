use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_phase::ViewSortedRenderPhases,
        render_resource::RenderPassDescriptor,
        renderer::{RenderContext, ViewQuery},
        view::ExtractedView,
    },
    ui_render::{TransparentUi, UiCameraView, UiViewTarget},
};

use crate::view_targets::MoonlyViewTargets;

#[derive(SystemParam)]
pub(crate) struct UiInputs<'w, 's> {
    ui_views: Query<'w, 's, (&'static ExtractedView, &'static UiViewTarget)>,
    targets: Query<'w, 's, (&'static MoonlyViewTargets, &'static ExtractedCamera)>,
    phases: Res<'w, ViewSortedRenderPhases<TransparentUi>>,
}

pub(crate) fn render_ui(
    world: &World,
    view: ViewQuery<&UiCameraView>,
    inputs: UiInputs,
    mut context: RenderContext,
) {
    let ui_view_entity = view.into_inner().0;
    let Ok((extracted_view, ui_view_target)) = inputs.ui_views.get(ui_view_entity) else {
        return;
    };
    let Ok((targets, camera)) = inputs.targets.get(ui_view_target.0) else {
        return;
    };
    let Some(phase) = inputs.phases.get(&extracted_view.retained_view_entity) else {
        return;
    };
    if phase.items.is_empty() {
        return;
    }
    let attachments = [Some(targets.post_process.get_unsampled_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_ui"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_ui");
    if let Some(viewport) = camera
        .viewport
        .clone()
        .map(crate::view_targets::private_viewport)
    {
        pass.set_camera_viewport(&viewport);
    }
    if let Err(error) = phase.render(&mut pass, world, ui_view_entity) {
        error!("Moonly UI phase failed: {error:?}");
    }
    span.end(&mut pass);
}

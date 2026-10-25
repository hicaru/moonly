use bevy::{
    camera::{MainPassResolutionOverride, Viewport},
    ecs::{
        schedule::{ScheduleBuildSettings, ScheduleLabel},
        system::SystemParam,
    },
    prelude::*,
    render::{
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_phase::ViewBinnedRenderPhases,
        render_resource::{RenderPassDescriptor, StoreOp},
        renderer::{RenderContext, ViewQuery},
        view::ExtractedView,
    },
};

use crate::{
    camera::MoonlyCamera,
    phase::{MoonlyAlphaMask3d, MoonlyOpaque3d},
    view_targets::MoonlyViewTargets,
};

/// Camera schedule for the Moonly deferred renderer.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct MoonlyRender;

pub(crate) fn base_schedule() -> Schedule {
    let mut schedule = Schedule::new(MoonlyRender);
    schedule.set_build_settings(ScheduleBuildSettings {
        auto_insert_apply_deferred: false,
        ..Default::default()
    });
    schedule
}

#[derive(SystemParam)]
pub(crate) struct GBufferRenderInputs<'w> {
    opaque_phases: Res<'w, ViewBinnedRenderPhases<MoonlyOpaque3d>>,
    alpha_phases: Res<'w, ViewBinnedRenderPhases<MoonlyAlphaMask3d>>,
    timer: Res<'w, crate::gpu_timer::MoonlyGpuTimer>,
}

pub(crate) fn render_gbuffer(
    world: &World,
    view: ViewQuery<
        (
            &ExtractedCamera,
            &ExtractedView,
            &MoonlyViewTargets,
            Option<&MainPassResolutionOverride>,
        ),
        With<MoonlyCamera>,
    >,
    inputs: GBufferRenderInputs,
    mut context: RenderContext,
) {
    let view_entity = view.entity();
    let (camera, extracted_view, targets, resolution_override) = view.into_inner();
    let (Some(opaque_phase), Some(alpha_phase)) = (
        inputs
            .opaque_phases
            .get(&extracted_view.retained_view_entity),
        inputs
            .alpha_phases
            .get(&extracted_view.retained_view_entity),
    ) else {
        return;
    };
    let attachments = [
        Some(targets.position.get_attachment()),
        Some(targets.normal.get_attachment()),
        Some(targets.albedo_gloss.get_attachment()),
        Some(targets.accumulator.get_attachment()),
    ];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_gbuffer"),
        color_attachments: &attachments,
        depth_stencil_attachment: Some(targets.depth.get_attachment(StoreOp::Store)),
        timestamp_writes: inputs
            .timer
            .timestamp_writes(crate::diagnostics::MoonlyPass::GBuffer),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_gbuffer");
    pass.set_stencil_reference(1);
    if let Some(viewport) =
        Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override)
    {
        pass.set_camera_viewport(&viewport);
    }
    render_phase(opaque_phase, &mut pass, world, view_entity, "opaque");
    render_phase(alpha_phase, &mut pass, world, view_entity, "alpha mask");
    span.end(&mut pass);
}

fn render_phase<'w, P: bevy::render::render_phase::BinnedPhaseItem>(
    phase: &bevy::render::render_phase::BinnedRenderPhase<P>,
    pass: &mut bevy::render::render_phase::TrackedRenderPass<'w>,
    world: &'w World,
    view: Entity,
    label: &str,
) {
    if let Err(error) = phase.render(pass, world, view) {
        error!("Moonly {label} G-buffer phase failed: {error:?}");
    }
}

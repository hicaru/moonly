use std::collections::HashSet;

use bevy::{
    prelude::*,
    render::{
        Extract,
        batching::gpu_preprocessing::{GpuPreprocessingMode, GpuPreprocessingSupport},
        render_phase::ViewBinnedRenderPhases,
        view::{NoIndirectDrawing, RetainedViewEntity},
    },
};

use crate::{
    camera::MoonlyCamera,
    phase::{MoonlyAlphaMask3d, MoonlyOpaque3d, MoonlyShadow3d},
};

type MoonlyCameraQuery = Query<
    'static,
    'static,
    (Entity, &'static Camera, Has<NoIndirectDrawing>),
    (With<Camera3d>, With<MoonlyCamera>),
>;

pub(crate) fn extract_moonly_camera_phases(
    mut opaque_phases: ResMut<ViewBinnedRenderPhases<MoonlyOpaque3d>>,
    mut alpha_mask_phases: ResMut<ViewBinnedRenderPhases<MoonlyAlphaMask3d>>,
    cameras: Extract<MoonlyCameraQuery>,
    mut live_views: Local<HashSet<RetainedViewEntity>>,
    gpu_support: Res<GpuPreprocessingSupport>,
) {
    live_views.clear();
    for (entity, camera, no_indirect_drawing) in &cameras {
        if !camera.is_active {
            continue;
        }
        let mode = gpu_support.min(if no_indirect_drawing {
            GpuPreprocessingMode::PreprocessingOnly
        } else {
            GpuPreprocessingMode::Culling
        });
        let view = RetainedViewEntity::new(entity.into(), None, 0);
        opaque_phases.prepare_for_new_frame(view, mode);
        alpha_mask_phases.prepare_for_new_frame(view, mode);
        live_views.insert(view);
    }
    opaque_phases.retain(|view, _| live_views.contains(view));
    alpha_mask_phases.retain(|view, _| live_views.contains(view));
}

pub(crate) fn extract_moonly_shadow_phases(
    mut phases: ResMut<ViewBinnedRenderPhases<MoonlyShadow3d>>,
    cameras: Extract<MoonlyCameraQuery>,
    mut live_views: Local<HashSet<RetainedViewEntity>>,
    gpu_support: Res<GpuPreprocessingSupport>,
) {
    live_views.clear();
    for (entity, camera, no_indirect_drawing) in &cameras {
        if !camera.is_active {
            continue;
        }
        let mode = gpu_support.min(if no_indirect_drawing {
            GpuPreprocessingMode::PreprocessingOnly
        } else {
            GpuPreprocessingMode::Culling
        });
        let view = RetainedViewEntity::new(entity.into(), None, 0);
        phases.prepare_for_new_frame(view, mode);
        live_views.insert(view);
    }
    phases.retain(|view, _| live_views.contains(view));
}

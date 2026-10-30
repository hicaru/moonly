use bevy::{
    camera::CameraOutputMode,
    core_pipeline::{
        blit::{BlitPipeline, BlitPipelineKey},
        tonemapping::{DebandDither, Tonemapping},
    },
    ecs::system::SystemParam,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_resource::{
            BlendState, CachedRenderPipelineId, PipelineCache, SpecializedRenderPipelines,
        },
        view::{ExtractedView, ResolvedCompositingSpace, ViewTarget},
    },
};

use crate::{camera::MoonlyCamera, view_targets::GBUFFER_FORMAT};

use super::{MoonlyNativePostPipeline, tonemap::MoonlyNativeTonemapKey};

#[derive(Component)]
pub(crate) struct ViewNativeTonemapPipeline(pub CachedRenderPipelineId);

#[derive(Component)]
pub(crate) struct ViewNativeUpscalePipeline(pub CachedRenderPipelineId);

#[derive(SystemParam)]
pub(crate) struct NativePipelinePrepareInputs<'w> {
    cache: ResMut<'w, PipelineCache>,
    tonemap_pipeline: Res<'w, MoonlyNativePostPipeline>,
    tonemap_pipelines: ResMut<'w, SpecializedRenderPipelines<MoonlyNativePostPipeline>>,
    blit_pipeline: Res<'w, BlitPipeline>,
    blit_pipelines: ResMut<'w, SpecializedRenderPipelines<BlitPipeline>>,
    native_post: Res<'w, moonly_core::MoonlyNativePost>,
}

type TonemapPipelineView = (
    Entity,
    &'static ExtractedView,
    &'static Tonemapping,
    &'static DebandDither,
);

type UpscalePipelineView = (
    Entity,
    &'static ViewTarget,
    &'static ExtractedCamera,
    Option<&'static ResolvedCompositingSpace>,
    Option<&'static crate::MoonlyPostSettings>,
);

pub(crate) fn prepare_native_post_pipelines(
    mut commands: Commands,
    mut inputs: NativePipelinePrepareInputs,
    tonemap_views: Query<TonemapPipelineView, With<MoonlyCamera>>,
    upscale_views: Query<UpscalePipelineView, With<MoonlyCamera>>,
) {
    for (entity, view, tonemapping, dither) in &tonemap_views {
        let key = MoonlyNativeTonemapKey {
            target_format: GBUFFER_FORMAT,
            deband_dither: *dither,
            tonemapping: *tonemapping,
            hue_rotate: view.color_grading.global.hue != 0.0,
            white_balance: view.color_grading.global.temperature != 0.0
                || view.color_grading.global.tint != 0.0,
            sectional_color_grading: view
                .color_grading
                .all_sections()
                .any(|section| *section != default()),
        };
        let id = inputs
            .tonemap_pipelines
            .specialize(&inputs.cache, &inputs.tonemap_pipeline, key);
        commands
            .entity(entity)
            .insert(ViewNativeTonemapPipeline(id));
    }
    for (entity, target, camera, resolved_space, settings) in &upscale_views {
        let Some(target_format) = target.out_texture_view_format() else {
            continue;
        };
        let blend_state = match camera.output_mode {
            CameraOutputMode::Skip => None,
            CameraOutputMode::Write { blend_state, .. } => blend_state.or_else(|| {
                (camera.sorted_camera_index_for_target > 0).then_some(BlendState::ALPHA_BLENDING)
            }),
        };
        let native = inputs.native_post.0 || settings.is_some_and(|settings| settings.native_post);
        let source_space = native
            .then(|| ResolvedCompositingSpace::space(resolved_space))
            .flatten();
        let key = BlitPipelineKey {
            target_format,
            blend_state,
            samples: 1,
            source_space,
        };
        let id = inputs
            .blit_pipelines
            .specialize(&inputs.cache, &inputs.blit_pipeline, key);
        inputs.cache.block_on_render_pipeline(id);
        commands
            .entity(entity)
            .insert(ViewNativeUpscalePipeline(id));
    }
}

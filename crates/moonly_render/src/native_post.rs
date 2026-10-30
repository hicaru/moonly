use bevy::{
    camera::{CameraOutputMode, ClearColor, ClearColorConfig},
    core_pipeline::{
        blit::BlitPipeline,
        tonemapping::{Tonemapping, TonemappingLuts, get_lut_bindings},
    },
    ecs::system::SystemParam,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_asset::RenderAssets,
        render_resource::{BindGroup, BindGroupEntries, PipelineCache, RenderPassDescriptor},
        renderer::{RenderContext, RenderDevice, ViewQuery},
        texture::{FallbackImage, GpuImage},
        view::{ViewTarget, ViewUniformOffset, ViewUniforms},
    },
};

use crate::{camera::MoonlyCamera, view_targets::MoonlyViewTargets};

mod hdr10;
mod pipelines;
mod tonemap;
pub(crate) use hdr10::{
    MoonlyHdr10OutputPipeline, init_hdr10_output_pipeline, prepare_hdr10_output_bind_groups,
    prepare_hdr10_output_pipelines,
};
use hdr10::{ViewHdr10OutputBindGroup, ViewHdr10OutputPipeline};
pub(crate) use pipelines::prepare_native_post_pipelines;
use pipelines::{ViewNativeTonemapPipeline, ViewNativeUpscalePipeline};
pub(crate) use tonemap::{MoonlyNativePostPipeline, init_native_post_pipeline};

#[derive(Component)]
pub(crate) struct ViewNativeTonemapBindGroup(BindGroup);

#[derive(Component)]
pub(crate) struct ViewNativeUpscaleBindGroup(BindGroup);

#[derive(SystemParam)]
pub(crate) struct NativePostPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyNativePostPipeline>,
    view_uniforms: Res<'w, ViewUniforms>,
    gpu_images: Res<'w, RenderAssets<GpuImage>>,
    fallback_image: Res<'w, FallbackImage>,
    tonemapping_luts: Res<'w, TonemappingLuts>,
    blit: Res<'w, BlitPipeline>,
}

pub(crate) fn prepare_native_post_bind_groups(
    mut commands: Commands,
    inputs: NativePostPrepareInputs,
    views: Query<
        (
            Entity,
            &MoonlyViewTargets,
            &Tonemapping,
            Option<&crate::MoonlyLinearOutput>,
        ),
        With<MoonlyCamera>,
    >,
) {
    for (entity, targets, tonemapping, linear) in &views {
        let lut = get_lut_bindings(
            &inputs.gpu_images,
            &inputs.tonemapping_luts,
            tonemapping,
            &inputs.fallback_image,
        );
        let tonemap = inputs.device.create_bind_group(
            Some("moonly_native_tonemap_bind_group"),
            &inputs
                .cache
                .get_bind_group_layout(&inputs.pipeline.tonemap_layout),
            &BindGroupEntries::sequential((
                &inputs.view_uniforms.uniforms,
                &targets.scene.texture.default_view,
                &inputs.pipeline.tonemap_sampler,
                lut.0,
                lut.1,
            )),
        );
        let output = if linear.is_some() {
            &targets.scene.texture.default_view
        } else {
            &targets.post_process.texture.default_view
        };
        let upscale = inputs
            .blit
            .create_bind_group(&inputs.device, output, &inputs.cache);
        commands.entity(entity).insert((
            ViewNativeTonemapBindGroup(tonemap),
            ViewNativeUpscaleBindGroup(upscale),
        ));
    }
}

type NativeBloomView = (
    &'static ViewTarget,
    &'static MoonlyViewTargets,
    &'static ExtractedCamera,
    &'static bevy::post_process::bloom::Bloom,
);

pub(crate) fn render_native_bloom_input(
    view: ViewQuery<NativeBloomView, With<MoonlyCamera>>,
    context: RenderContext,
) {
    copy_native_bloom(view, context, true);
}

pub(crate) fn render_native_bloom_output(
    view: ViewQuery<NativeBloomView, With<MoonlyCamera>>,
    context: RenderContext,
) {
    copy_native_bloom(view, context, false);
}

fn copy_native_bloom(
    view: ViewQuery<NativeBloomView, With<MoonlyCamera>>,
    mut context: RenderContext,
    input: bool,
) {
    let (target, targets, camera, _) = view.into_inner();
    if target.main_texture_format() != crate::view_targets::GBUFFER_FORMAT {
        return;
    }
    let Some(size) = camera.physical_viewport_size else {
        return;
    };
    let size = bevy::image::ToExtents::to_extents(size);
    let scene = targets.scene.texture.texture.as_image_copy();
    let main = target.main_texture().as_image_copy();
    if input {
        context
            .command_encoder()
            .copy_texture_to_texture(scene, main, size);
    } else {
        context
            .command_encoder()
            .copy_texture_to_texture(main, scene, size);
    }
}

type NativeTonemapView = (
    &'static ExtractedCamera,
    &'static ViewUniformOffset,
    &'static MoonlyViewTargets,
    &'static ViewNativeTonemapPipeline,
    &'static Tonemapping,
    &'static ViewNativeTonemapBindGroup,
    Option<&'static crate::MoonlyPostSettings>,
);

pub(crate) fn render_tonemapping(
    view: ViewQuery<NativeTonemapView, With<MoonlyCamera>>,
    cache: Res<PipelineCache>,
    native_post: Res<moonly_core::MoonlyNativePost>,
    mut context: RenderContext,
) {
    let (camera, view_offset, targets, view_pipeline, tonemapping, group, settings) =
        view.into_inner();
    let native = native_post.0 || settings.is_some_and(|settings| settings.native_post);
    if !native || !tonemapping.is_enabled() || !camera.hdr {
        return;
    }
    let Some(pipeline) = cache.get_render_pipeline(view_pipeline.0) else {
        return;
    };
    let attachments = [Some(targets.post_process.get_unsampled_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_native_tonemapping"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_native_tonemapping");
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &group.0, &[view_offset.offset]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

type NativeUpscaleView = (
    &'static ViewTarget,
    &'static ViewNativeUpscalePipeline,
    &'static ExtractedCamera,
    &'static ViewNativeUpscaleBindGroup,
    Option<&'static ViewHdr10OutputPipeline>,
    Option<&'static ViewHdr10OutputBindGroup>,
);

pub(crate) fn render_upscaling(
    view: ViewQuery<NativeUpscaleView, With<MoonlyCamera>>,
    cache: Res<PipelineCache>,
    clear_color_global: Res<ClearColor>,
    mut context: RenderContext,
) {
    let (target, view_pipeline, camera, group, hdr10_pipeline, hdr10_group) = view.into_inner();
    let clear_color = match camera.output_mode {
        CameraOutputMode::Write { clear_color, .. } => clear_color,
        CameraOutputMode::Skip => return,
    };
    let clear_color = if camera.sorted_camera_index_for_target == 0 {
        match clear_color {
            ClearColorConfig::Default => Some(clear_color_global.0),
            ClearColorConfig::Custom(color) => Some(color),
            ClearColorConfig::None => None,
        }
    } else {
        None
    };
    let Some(attachment) = target.out_texture_color_attachment(clear_color.map(Into::into)) else {
        return;
    };
    let (pipeline, output_group) = if let Some(hdr10_pipeline) = hdr10_pipeline {
        let Some(group) = hdr10_group else {
            return;
        };
        let Some(pipeline) = cache.get_render_pipeline(hdr10_pipeline.0) else {
            return;
        };
        (pipeline, &group.0)
    } else {
        let Some(pipeline) = cache.get_render_pipeline(view_pipeline.0) else {
            return;
        };
        (pipeline, &group.0)
    };
    let attachments = [Some(attachment)];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_native_upscaling"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_native_upscaling");
    if let Some(viewport) = &camera.viewport {
        pass.set_camera_viewport(viewport);
    }
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, output_group, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

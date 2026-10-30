use core::ops::Range;

use bevy::{
    camera::{MainPassResolutionOverride, Viewport},
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    image::ToExtents,
    pbr::{ScreenSpaceTransmission, Transmissive3d, ViewTransmissionTexture},
    prelude::*,
    render::{
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::{
            RenderPassDescriptor, StoreOp, TextureDescriptor, TextureDimension, TextureUsages,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        texture::TextureCache,
        view::{ExtractedView, ViewDepthStencilTexture},
    },
};

use crate::{camera::MoonlyCamera, view_targets::MoonlyViewTargets};

type TransmissionViewQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ExtractedCamera,
        &'static ScreenSpaceTransmission,
        &'static ExtractedView,
    ),
    With<MoonlyCamera>,
>;

type TransmissiveViewQuery<'w, 's> = ViewQuery<
    'w,
    's,
    (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static ScreenSpaceTransmission,
        &'static MoonlyViewTargets,
        &'static crate::light_packages::MoonlyViewLightPackage,
        Option<&'static ViewTransmissionTexture>,
        &'static ViewDepthStencilTexture,
        Option<&'static MainPassResolutionOverride>,
    ),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct TransmissionPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    cache: ResMut<'w, TextureCache>,
    opaque: Res<'w, ViewBinnedRenderPhases<Opaque3d>>,
    alpha: Res<'w, ViewBinnedRenderPhases<AlphaMask3d>>,
    transparent: Res<'w, ViewSortedRenderPhases<Transparent3d>>,
    transmissive: Res<'w, ViewSortedRenderPhases<Transmissive3d>>,
}

pub(crate) fn prepare_transmission_textures(
    mut commands: Commands,
    mut inputs: TransmissionPrepareInputs,
    views: TransmissionViewQuery,
) {
    for (entity, camera, settings, view) in &views {
        let ready = inputs.opaque.contains_key(&view.retained_view_entity)
            && inputs.alpha.contains_key(&view.retained_view_entity)
            && inputs.transparent.contains_key(&view.retained_view_entity);
        let has_items = inputs
            .transmissive
            .get(&view.retained_view_entity)
            .is_some_and(|phase| !phase.items.is_empty());
        let Some(size) = camera.physical_viewport_size else {
            continue;
        };
        if !ready {
            continue;
        }
        if !has_items || settings.steps == 0 {
            commands.entity(entity).remove::<ViewTransmissionTexture>();
            continue;
        }
        let cached = inputs.cache.get(
            &inputs.device,
            TextureDescriptor {
                label: Some("moonly_transmission_texture"),
                size: size.to_extents(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: view.target_format,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            },
        );
        let sampler =
            inputs
                .device
                .create_sampler(&bevy::render::render_resource::SamplerDescriptor {
                    label: Some("moonly_transmission_sampler"),
                    mag_filter: bevy::render::render_resource::FilterMode::Linear,
                    min_filter: bevy::render::render_resource::FilterMode::Linear,
                    ..Default::default()
                });
        commands.entity(entity).insert(ViewTransmissionTexture {
            texture: cached.texture,
            view: cached.default_view,
            sampler,
        });
    }
}

pub(crate) fn render_transmissive(
    world: &World,
    view: TransmissiveViewQuery,
    phases: Res<ViewSortedRenderPhases<Transmissive3d>>,
    mut context: RenderContext,
) {
    let view_entity = view.entity();
    let (
        camera,
        extracted_view,
        settings,
        targets,
        package,
        transmission,
        depth,
        resolution_override,
    ) = view.into_inner();
    if package.0.has_non_native() {
        return;
    }
    let Some(phase) = phases.get(&extracted_view.retained_view_entity) else {
        return;
    };
    let Some(size) = camera.physical_viewport_size else {
        return;
    };
    if phase.items.is_empty() || (settings.steps > 0 && transmission.is_none()) {
        return;
    }
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    for range in split_range(0..phase.items.len(), settings.steps.max(1)) {
        if settings.steps > 0
            && let Some(transmission) = transmission
        {
            context.command_encoder().copy_texture_to_texture(
                targets.scene.texture.texture.as_image_copy(),
                transmission.texture.as_image_copy(),
                size.to_extents(),
            );
        }
        let attachments = [Some(targets.scene.get_attachment())];
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("moonly_transmissive"),
            color_attachments: &attachments,
            depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let span = diagnostics.pass_span(&mut pass, "moonly_transmissive");
        if let Some(viewport) =
            Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override)
                .map(crate::view_targets::private_viewport)
        {
            pass.set_camera_viewport(&viewport);
        }
        if let Err(error) = phase.render_range(&mut pass, world, view_entity, range) {
            error!("Moonly transmissive phase failed: {error:?}");
        }
        span.end(&mut pass);
    }
}

fn split_range(range: Range<usize>, max_num_splits: usize) -> impl Iterator<Item = Range<usize>> {
    let len = range.end - range.start;
    let num_splits = max_num_splits.min(len).max(1);
    let step = len / num_splits;
    let mut rem = len % num_splits;
    let mut start = range.start;
    (0..num_splits).map(move |_| {
        let extra = if rem > 0 {
            rem -= 1;
            1
        } else {
            0
        };
        let end = (start + step + extra).min(range.end);
        let result = start..end;
        start = end;
        result
    })
}

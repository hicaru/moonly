use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::{
        render_resource::{
            Extent3d, PipelineCache, TextureDescriptor, TextureDimension, TextureUsages,
            UniformBuffer,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::{DepthStencilAttachment, TextureCache},
        view::ExtractedView,
    },
};
use moonly_core::{
    LightPackage, MoonlyLight, MoonlyLightType, MoonlyShadowBatch, MoonlyShadowPackage,
    MoonlyShadowRect, MoonlyShadowView, MoonlySunCascade, MoonlySunPackage, SpotShadowXform,
};

pub(crate) use crate::shadow_target_types::{
    MoonlyShadowBatchTarget, MoonlyShadowSample, MoonlyShadowViewIndex, MoonlyShadowViewTarget,
    MoonlySpotShadowTarget,
};
use crate::{
    camera::MoonlyCamera,
    minmax::{MoonlyMinMaxTarget, minmax_descriptor},
    occlusion_state::{MoonlyLightKey, MoonlyOcclusionState},
    shadow_pipeline::{MoonlyShadowPipeline, MoonlyShadowViewUniform},
    view_targets::DEPTH_FORMAT,
};

const MIN_ATLAS_SIZE: u32 = 64;
const MAX_ATLAS_SIZE: u32 = 2048;

#[derive(SystemParam)]
pub(crate) struct ShadowTargetInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    cache: ResMut<'w, TextureCache>,
    pipeline: Res<'w, MoonlyShadowPipeline>,
    pipeline_cache: Res<'w, PipelineCache>,
    package: Res<'w, LightPackage>,
    shadows: ResMut<'w, MoonlyShadowPackage>,
    sun: ResMut<'w, MoonlySunPackage>,
}

type ShadowCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ExtractedView,
        Option<&'static MoonlyOcclusionState>,
    ),
    With<MoonlyCamera>,
>;

pub(crate) fn prepare_spot_shadow_targets(
    mut commands: Commands,
    mut inputs: ShadowTargetInputs,
    views: ShadowCameraQuery,
) {
    let shadow_batches = std::mem::take(&mut inputs.shadows.batches);
    let sun_cascades = std::mem::take(&mut inputs.sun.cascades);
    let direct_light = inputs.package.v_direct.first().cloned();
    for (entity, camera, occlusion) in &views {
        let camera_position = camera.world_from_view.translation();
        let fallback_xform = package_spot(&inputs.package)
            .map_or_else(SpotShadowXform::default, |light| {
                SpotShadowXform::compute_xf_spot(light, camera_position, MAX_ATLAS_SIZE, 0)
            });
        let atlas_size = if shadow_batches.is_empty() {
            MAX_ATLAS_SIZE
        } else {
            inputs.shadows.pool_size.max(MIN_ATLAS_SIZE)
        };
        let mut batches = Vec::new();
        for batch in &shadow_batches {
            batches.push(prepare_batch(
                &mut inputs,
                batch,
                atlas_size,
                camera,
                occlusion,
            ));
        }
        if batches.is_empty() {
            batches.push(prepare_fallback_batch(
                &mut inputs,
                fallback_xform,
                atlas_size,
            ));
        }
        let mut sun_batches = Vec::new();
        let mut near_minmax = None;
        if let Some(light) = direct_light.as_ref() {
            let sun_atlas_size = inputs.sun.pool_size.max(MIN_ATLAS_SIZE);
            for (index, cascade) in sun_cascades.iter().enumerate() {
                let create_minmax = index == 0 && inputs.sun.use_minmax_near;
                let (batch, minmax) =
                    prepare_sun_batch(&mut inputs, cascade, light, sun_atlas_size, create_minmax);
                sun_batches.push(batch);
                if minmax.is_some() {
                    near_minmax = minmax;
                }
            }
        }
        commands.entity(entity).insert((
            MoonlySpotShadowTarget {
                batches,
                sun_batches,
                near_minmax,
                atlas_size,
                fallback_xform,
            },
            MoonlyShadowViewIndex::default(),
        ));
    }
    inputs.shadows.batches = shadow_batches;
    inputs.sun.cascades = sun_cascades;
}

fn prepare_batch(
    inputs: &mut ShadowTargetInputs,
    batch: &MoonlyShadowBatch,
    atlas_size: u32,
    camera: &ExtractedView,
    occlusion: Option<&MoonlyOcclusionState>,
) -> MoonlyShadowBatchTarget {
    let texture = inputs
        .cache
        .get(&inputs.device, atlas_descriptor(atlas_size));
    let depth = DepthStencilAttachment::new(texture, None, Some(1.0), None);
    let mut views = Vec::with_capacity(batch.views.len());
    for view in &batch.views {
        let Some(light) = inputs.package.v_shadowed.get(view.light_index) else {
            continue;
        };
        let key = MoonlyLightKey {
            source_entity: light.source_entity,
            omnipart_face: light.omnipart_face,
        };
        let occluded = occlusion.is_some_and(|state| !state.is_visible(key));
        if occluded || !crate::visibility::light_visible(light, camera) {
            continue;
        }
        views.push(prepare_view(inputs, view, light));
    }
    MoonlyShadowBatchTarget { depth, views }
}

fn prepare_sun_batch(
    inputs: &mut ShadowTargetInputs,
    cascade: &MoonlySunCascade,
    light: &MoonlyLight,
    atlas_size: u32,
    create_minmax: bool,
) -> (MoonlyShadowBatchTarget, Option<MoonlyMinMaxTarget>) {
    let texture = inputs
        .cache
        .get(&inputs.device, atlas_descriptor(atlas_size));
    let depth = DepthStencilAttachment::new(texture, None, Some(1.0), None);
    let views = vec![prepare_view(inputs, &cascade.view, light)];
    let minmax = create_minmax.then(|| {
        let size = (atlas_size / 4).max(1);
        MoonlyMinMaxTarget {
            texture: inputs.cache.get(&inputs.device, minmax_descriptor(size)),
            size,
            bind_group: None,
        }
    });
    (MoonlyShadowBatchTarget { depth, views }, minmax)
}

fn prepare_fallback_batch(
    inputs: &mut ShadowTargetInputs,
    xform: SpotShadowXform,
    atlas_size: u32,
) -> MoonlyShadowBatchTarget {
    let texture = inputs
        .cache
        .get(&inputs.device, atlas_descriptor(atlas_size));
    let depth = DepthStencilAttachment::new(texture, None, Some(1.0), None);
    let mut views = Vec::new();
    if let Some(light) = package_spot(&inputs.package) {
        let size = xform.smap_size.clamp(MIN_ATLAS_SIZE, atlas_size);
        let view = MoonlyShadowView {
            light_index: 0,
            xform,
            rect: MoonlyShadowRect { x: 0, y: 0, size },
        };
        views.push(prepare_view(inputs, &view, light));
    }
    MoonlyShadowBatchTarget { depth, views }
}

fn prepare_view(
    inputs: &ShadowTargetInputs,
    view: &MoonlyShadowView,
    light: &MoonlyLight,
) -> MoonlyShadowViewTarget {
    let mut uniform = UniformBuffer::from(MoonlyShadowViewUniform::from(&view.xform));
    uniform.write_buffer(&inputs.device, &inputs.queue);
    let bind_group =
        inputs
            .pipeline
            .create_view_bind_group(&inputs.device, &inputs.pipeline_cache, &uniform);
    MoonlyShadowViewTarget {
        source_entity: light.source_entity,
        omnipart_face: light.omnipart_face,
        xform: view.xform,
        rect: view.rect,
        bind_group,
    }
}

fn package_spot(package: &LightPackage) -> Option<&MoonlyLight> {
    package
        .v_shadowed
        .iter()
        .find(|light| light.light_type == MoonlyLightType::Spot)
}

fn atlas_descriptor(size: u32) -> TextureDescriptor<'static> {
    TextureDescriptor {
        label: Some("moonly_spot_shadow_atlas"),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }
}

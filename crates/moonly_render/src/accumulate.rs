use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{BindGroup, BindGroupEntries, PipelineCache, UniformBuffer},
        renderer::{RenderDevice, RenderQueue},
        sync_world::MainEntity,
        texture::GpuImage,
        view::ExtractedView,
    },
};
use moonly_core::{
    LightPackage, MoonlyLight, MoonlyLightFlags, MoonlyLightType, MoonlyProjector,
    VirtualGiLightPackage,
};

use crate::{
    camera::MoonlyCamera, projector::MoonlyFallbackProjector,
    shadow_targets::MoonlySpotShadowTarget, spot_pipeline::MoonlySpotPipeline,
    view_targets::MoonlyViewTargets,
};

pub(crate) mod spot_uniform;
pub(crate) use self::spot_uniform::MoonlySpotUniform;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalVolumeKind {
    Cone,
    Sphere,
}

pub(crate) struct MoonlySpotBindGroup {
    pub bind_group: BindGroup,
    pub kind: LocalVolumeKind,
    pub volumetric: bool,
    pub key: crate::occlusion_state::MoonlyLightKey,
}

#[derive(Component, Default)]
pub(crate) struct MoonlySpotBindGroups(pub Vec<MoonlySpotBindGroup>);

#[derive(SystemParam)]
pub(crate) struct SpotPrepareInputs<'w> {
    pub(crate) device: Res<'w, RenderDevice>,
    pub(crate) queue: Res<'w, RenderQueue>,
    pub(crate) pipeline_cache: Res<'w, PipelineCache>,
    pub(crate) pipeline: Res<'w, MoonlySpotPipeline>,
    pub(crate) native_pipeline: Res<'w, crate::occlusion::native::MoonlyNativeLocalPipeline>,
    pub(crate) virtual_lights: Res<'w, VirtualGiLightPackage>,
    pub(crate) images: Res<'w, RenderAssets<GpuImage>>,
    pub(crate) fallback: Res<'w, MoonlyFallbackProjector>,
    pub(crate) lut: Res<'w, crate::material_lut::MoonlyMaterialLutTexture>,
    pub(crate) jitter: Res<'w, crate::jitter::MoonlyJitterTexture>,
}

pub(crate) struct SpotViewContext<'a, 'w> {
    pub(crate) inputs: &'a SpotPrepareInputs<'w>,
    pub(crate) view: &'a ExtractedView,
    pub(crate) layers: &'a bevy::camera::visibility::RenderLayers,
    pub(crate) targets: &'a MoonlyViewTargets,
    pub(crate) shadow: &'a MoonlySpotShadowTarget,
    pub(crate) package: &'a LightPackage,
}

type MoonlySpotViewQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ExtractedView,
        Option<&'static bevy::camera::visibility::RenderLayers>,
        &'static MoonlyViewTargets,
        &'static MoonlySpotShadowTarget,
        &'static crate::light_packages::MoonlyViewLightPackage,
        Option<&'static mut MoonlySpotBindGroups>,
        Option<&'static mut crate::occlusion::native::MoonlyNativeLocalBindGroup>,
    ),
    With<MoonlyCamera>,
>;

pub(crate) fn prepare_spot_bind_groups(
    mut commands: Commands,
    inputs: SpotPrepareInputs,
    mut views: MoonlySpotViewQuery,
    projectors: Query<(&MainEntity, &MoonlyProjector)>,
    mut scratch: Local<crate::occlusion::native::bins::NativeBuildScratch>,
) {
    let default_layers = bevy::camera::visibility::RenderLayers::default();
    for (entity, view, layers, targets, shadow, package, existing, native_existing) in &mut views {
        let context = SpotViewContext {
            inputs: &inputs,
            view,
            layers: layers.unwrap_or(&default_layers),
            targets,
            shadow,
            package: &package.0,
        };
        if crate::occlusion::native::can_batch(&context, &projectors) {
            commands.entity(entity).remove::<(
                MoonlySpotBindGroups,
                crate::occlusion_state::MoonlyOcclusionState,
            )>();
            let native = crate::occlusion::native::prepare_native_local(
                &context,
                &projectors,
                native_existing.map(Mut::into_inner),
                &mut scratch,
            );
            if let Some(native) = native {
                commands.entity(entity).insert(native);
            } else {
                commands
                    .entity(entity)
                    .remove::<crate::occlusion::native::MoonlyNativeLocalBindGroup>();
            }
            continue;
        }
        commands
            .entity(entity)
            .remove::<crate::occlusion::native::MoonlyNativeLocalBindGroup>();
        if let Some(mut existing) = existing {
            fill_spot_groups(&context, &projectors, &mut existing.0);
            continue;
        }
        let mut groups = Vec::new();
        fill_spot_groups(&context, &projectors, &mut groups);
        commands.entity(entity).insert(MoonlySpotBindGroups(groups));
    }
}

fn fill_spot_groups(
    context: &SpotViewContext,
    projectors: &Query<(&MainEntity, &MoonlyProjector)>,
    groups: &mut Vec<MoonlySpotBindGroup>,
) {
    groups.clear();
    let package = context.package;
    let unshadowed = package.v_point.iter().chain(package.v_spot.iter());
    for light in unshadowed.chain(&package.v_shadowed) {
        if !crate::visibility::light_visible(light, context.view) {
            continue;
        }
        let projector = find_projector(light, context.inputs, projectors);
        let key = crate::occlusion_state::MoonlyLightKey {
            source_entity: light.source_entity,
            omnipart_face: light.omnipart_face,
            virtual_index: None,
        };
        groups.extend(create_spot_bind_group_with_key(
            context, light, projector, key,
        ));
    }
    for (index, virtual_light) in context.inputs.virtual_lights.lights.iter().enumerate() {
        let light = crate::reflected_lights::local_light(virtual_light);
        if virtual_light.visible_in_layers(context.layers)
            && crate::visibility::light_visible(&light, context.view)
        {
            groups.extend(create_spot_bind_group_with_key(
                context,
                &light,
                None,
                crate::occlusion_state::MoonlyLightKey::virtual_light(index),
            ));
        }
    }
}

fn create_spot_bind_group_with_key(
    context: &SpotViewContext,
    light: &MoonlyLight,
    projector: Option<&GpuImage>,
    key: crate::occlusion_state::MoonlyLightKey,
) -> Option<MoonlySpotBindGroup> {
    let inputs = context.inputs;
    let sample = context.shadow.sample(light);
    let depth = sample
        .as_ref()
        .map(|sample| sample.depth)
        .or_else(|| context.shadow.first_depth())?;
    let projector_view = projector.map_or(&inputs.fallback.view, |image| &image.texture_view);
    let projector_sampler = projector.map_or(&inputs.fallback.sampler, |image| &image.sampler);
    let mut uniform = UniformBuffer::from(MoonlySpotUniform::new(
        light,
        context.view,
        sample.as_ref(),
        projector.is_some(),
    ));
    uniform.write_buffer(&inputs.device, &inputs.queue);
    let bind_group = inputs.device.create_bind_group(
        Some("moonly_spot_bind_group"),
        &inputs
            .pipeline_cache
            .get_bind_group_layout(&inputs.pipeline.layout),
        &BindGroupEntries::sequential((
            &context.targets.position.texture.default_view,
            &context.targets.normal.texture.default_view,
            &context.targets.albedo_gloss.texture.default_view,
            &uniform,
            &inputs.lut.view,
            &inputs.lut.sampler,
            depth,
            &inputs.pipeline.shadow_sampler,
            projector_view,
            projector_sampler,
            &inputs.jitter.view,
            &inputs.jitter.sampler,
        )),
    );
    let kind = if light.light_type == MoonlyLightType::Point {
        LocalVolumeKind::Sphere
    } else {
        LocalVolumeKind::Cone
    };
    Some(MoonlySpotBindGroup {
        bind_group,
        kind,
        volumetric: light.flags.contains(MoonlyLightFlags::VOLUMETRIC)
            && light.volumetric.intensity > 0.0,
        key,
    })
}

pub(crate) fn find_projector<'a>(
    light: &MoonlyLight,
    inputs: &'a SpotPrepareInputs<'_>,
    projectors: &Query<(&MainEntity, &MoonlyProjector)>,
) -> Option<&'a GpuImage> {
    light
        .source_entity
        .and_then(|source| {
            projectors
                .iter()
                .find(|(main, _)| main.id() == source)
                .map(|(_, projector)| projector)
        })
        .and_then(|projector| inputs.images.get(&projector.texture))
}

use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{BindGroup, BindGroupEntries, PipelineCache, ShaderType, UniformBuffer},
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
    camera::MoonlyCamera,
    light_volume::clip_from_local,
    material_lut::MoonlyMaterialLutTexture,
    projector::MoonlyFallbackProjector,
    shadow_targets::{MoonlyShadowSample, MoonlySpotShadowTarget},
    spot_pipeline::MoonlySpotPipeline,
    view_targets::MoonlyViewTargets,
};

#[derive(ShaderType)]
pub(crate) struct MoonlySpotUniform {
    position_range: Vec4,
    direction_cos: Vec4,
    color: Vec4,
    view_position: Vec4,
    shadow_from_world: Mat4,
    projector_from_world: Mat4,
    clip_from_local: Mat4,
    shadow_settings: Vec4,
    shadow_transform: Vec4,
    world_from_clip: Mat4,
    volumetric: Vec4,
    screen_size: Vec4,
}

impl MoonlySpotUniform {
    fn new(
        light: &MoonlyLight,
        view: &ExtractedView,
        shadow: Option<&MoonlyShadowSample>,
        has_projector: bool,
    ) -> Self {
        let camera_position = view.world_from_view.translation();
        let atlas_size = shadow.map_or(2048, |shadow| shadow.atlas_size);
        let rect = shadow.map_or(
            moonly_core::MoonlyShadowRect {
                x: 0,
                y: 0,
                size: atlas_size,
            },
            |shadow| shadow.rect,
        );
        let projector =
            moonly_core::SpotShadowXform::compute_xf_spot(light, camera_position, atlas_size, 0);
        let clip_from_world = view.clip_from_view * view.world_from_view.to_matrix().inverse();
        let lowres_size = Vec2::new(
            (view.viewport.z / 8).max(1) as f32,
            (view.viewport.w / 8).max(1) as f32,
        );
        Self {
            position_range: light.position.extend(light.range),
            direction_cos: light
                .direction
                .extend(if light.light_type == MoonlyLightType::Point {
                    -1.0
                } else {
                    (light.cone_angle * 0.5).cos()
                }),
            color: light.color.extend(1.0),
            view_position: camera_position.extend(1.0),
            shadow_from_world: shadow.map_or(Mat4::IDENTITY, |shadow| shadow.xform.combine),
            projector_from_world: projector.combine,
            clip_from_local: clip_from_local(light, view),
            shadow_settings: Vec4::new(
                if shadow.is_some_and(|shadow| shadow.enabled) {
                    1.0
                } else {
                    0.0
                },
                0.0005,
                1.0 / rect.size as f32,
                if has_projector { 1.0 } else { 0.0 },
            ),
            shadow_transform: Vec4::new(
                rect.x as f32 / atlas_size as f32,
                rect.y as f32 / atlas_size as f32,
                rect.size as f32 / atlas_size as f32,
                0.0,
            ),
            world_from_clip: clip_from_world.inverse(),
            volumetric: Vec4::new(
                light.volumetric.intensity,
                light.volumetric.distance,
                light.volumetric.quality,
                if light.inner_cone_angle >= 0.0 {
                    (light.inner_cone_angle * 0.5).cos()
                } else {
                    -1.0
                },
            ),
            screen_size: lowres_size.extend(0.0).extend(0.0),
        }
    }
}

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
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    pipeline_cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlySpotPipeline>,
    virtual_lights: Res<'w, VirtualGiLightPackage>,
    lut: Res<'w, MoonlyMaterialLutTexture>,
    images: Res<'w, RenderAssets<GpuImage>>,
    fallback: Res<'w, MoonlyFallbackProjector>,
    jitter: Res<'w, crate::jitter::MoonlyJitterTexture>,
}

struct SpotViewContext<'a, 'w> {
    inputs: &'a SpotPrepareInputs<'w>,
    view: &'a ExtractedView,
    targets: &'a MoonlyViewTargets,
    shadow: &'a MoonlySpotShadowTarget,
    package: &'a LightPackage,
}

type MoonlySpotViewQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ExtractedView,
        &'static MoonlyViewTargets,
        &'static MoonlySpotShadowTarget,
        &'static crate::light_packages::MoonlyViewLightPackage,
        Option<&'static mut MoonlySpotBindGroups>,
    ),
    With<MoonlyCamera>,
>;

pub(crate) fn prepare_spot_bind_groups(
    mut commands: Commands,
    inputs: SpotPrepareInputs,
    mut views: MoonlySpotViewQuery,
    projectors: Query<(&MainEntity, &MoonlyProjector)>,
) {
    for (entity, view, targets, shadow, package, existing) in &mut views {
        let context = SpotViewContext {
            inputs: &inputs,
            view,
            targets,
            shadow,
            package: &package.0,
        };
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
        groups.extend(create_spot_bind_group(context, light, projector));
    }
    for virtual_light in &context.inputs.virtual_lights.lights {
        let light = crate::reflected_lights::local_light(virtual_light);
        if crate::visibility::light_visible(&light, context.view) {
            groups.extend(create_spot_bind_group(context, &light, None));
        }
    }
}

fn create_spot_bind_group(
    context: &SpotViewContext,
    light: &MoonlyLight,
    projector: Option<&GpuImage>,
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
        key: crate::occlusion_state::MoonlyLightKey {
            source_entity: light.source_entity,
            omnipart_face: light.omnipart_face,
        },
    })
}

fn find_projector<'a>(
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

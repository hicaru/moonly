use bevy::{
    asset::AssetServer,
    core_pipeline::FullscreenShader,
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState,
            PipelineCache, RenderPassDescriptor, RenderPipelineDescriptor, SamplerBindingType,
            ShaderStages, ShaderType, StorageBuffer, TextureSampleType, UniformBuffer,
            binding_types::{
                sampler, storage_buffer_read_only, texture_2d, texture_2d_array, texture_3d,
                uniform_buffer,
            },
        },
        renderer::{RenderContext, ViewQuery},
        sync_world::MainEntity,
    },
};
use moonly_core::{LightPackage, MoonlyLightFlags, MoonlyProjector};

use crate::{
    accumulate::{MoonlySpotUniform, SpotViewContext, find_projector},
    camera::MoonlyCamera,
    spot_pipeline::additive_component,
    view_targets::{GBUFFER_FORMAT, MoonlyViewTargets},
};

pub(crate) mod bins;

#[derive(Resource)]
pub(crate) struct MoonlyNativeLocalPipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedRenderPipelineId,
}

#[derive(Component)]
pub(crate) struct MoonlyNativeLocalBindGroup {
    bind_group: BindGroup,
    storage: StorageBuffer<Vec<MoonlySpotUniform>>,
    tiles: StorageBuffer<Vec<bins::NativeTile>>,
    indices: StorageBuffer<Vec<u32>>,
}

#[derive(ShaderType)]
struct MoonlyNativeLocalParams {
    grid: UVec4,
}

pub(crate) fn init_native_local_pipeline(
    mut commands: Commands,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    cache: Res<PipelineCache>,
) {
    let layout = native_layout();
    let shader = asset_server.load("embedded://moonly_render/accum_native.wesl");
    let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("moonly_native_local".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader,
            targets: vec![Some(ColorTargetState {
                format: GBUFFER_FORMAT,
                blend: Some(BlendState {
                    color: additive_component(),
                    alpha: additive_component(),
                }),
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(MoonlyNativeLocalPipeline { layout, id });
}

fn native_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "moonly_native_local_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                storage_buffer_read_only::<MoonlySpotUniform>(false),
                texture_3d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Depth),
                sampler(SamplerBindingType::Comparison),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<MoonlyNativeLocalParams>(false),
                storage_buffer_read_only::<bins::NativeTile>(false),
                storage_buffer_read_only::<u32>(false),
                texture_2d(TextureSampleType::Float { filterable: true }),
            ),
        ),
    )
}

pub(crate) fn can_batch(
    context: &SpotViewContext,
    projectors: &Query<(&MainEntity, &MoonlyProjector)>,
) -> bool {
    projectors.is_empty()
        && native_batch_eligible(
            context.package,
            !context.inputs.virtual_lights.lights.is_empty(),
        )
}

fn native_batch_eligible(package: &LightPackage, has_virtual: bool) -> bool {
    !has_virtual
        && package.total_count() > 0
        && package
            .v_point
            .iter()
            .chain(&package.v_spot)
            .chain(&package.v_shadowed)
            .all(|light| light.flags.contains(MoonlyLightFlags::NATIVE))
}

pub(crate) fn prepare_native_local(
    context: &SpotViewContext,
    projectors: &Query<(&MainEntity, &MoonlyProjector)>,
    existing: Option<&mut MoonlyNativeLocalBindGroup>,
    scratch: &mut bins::NativeBuildScratch,
) -> Option<MoonlyNativeLocalBindGroup> {
    fill_native_uniforms(context, projectors, scratch);
    if scratch.lights.is_empty() {
        return None;
    }
    let (mut storage, mut tiles, mut indices) = match existing {
        Some(group) => (
            std::mem::replace(&mut group.storage, StorageBuffer::from(Vec::new())),
            std::mem::replace(&mut group.tiles, StorageBuffer::from(Vec::new())),
            std::mem::replace(&mut group.indices, StorageBuffer::from(Vec::new())),
        ),
        None => (
            StorageBuffer::from(Vec::new()),
            StorageBuffer::from(Vec::new()),
            StorageBuffer::from(Vec::new()),
        ),
    };
    let grid = bins::build(context.view, scratch);
    storage.get_mut().clear();
    storage.get_mut().extend_from_slice(&scratch.lights);
    storage.write_buffer(&context.inputs.device, &context.inputs.queue);
    tiles.get_mut().clear();
    tiles.get_mut().extend_from_slice(&scratch.tiles);
    tiles.write_buffer(&context.inputs.device, &context.inputs.queue);
    indices.get_mut().clear();
    indices.get_mut().extend_from_slice(&scratch.indices);
    indices.write_buffer(&context.inputs.device, &context.inputs.queue);
    let mut params = UniformBuffer::from(MoonlyNativeLocalParams { grid });
    params.write_buffer(&context.inputs.device, &context.inputs.queue);
    let depth = context.shadow.first_depth()?;
    let bind_group = context.inputs.device.create_bind_group(
        Some("moonly_native_local_bind_group"),
        &context
            .inputs
            .pipeline_cache
            .get_bind_group_layout(&context.inputs.native_pipeline.layout),
        &BindGroupEntries::sequential((
            &context.targets.position.texture.default_view,
            &context.targets.normal.texture.default_view,
            &context.targets.albedo_gloss.texture.default_view,
            &storage,
            &context.inputs.lut.view,
            &context.inputs.lut.sampler,
            depth,
            &context.inputs.pipeline.shadow_sampler,
            &context.inputs.fallback.view,
            &context.inputs.fallback.sampler,
            &context.inputs.jitter.view,
            &context.inputs.jitter.sampler,
            &params,
            &tiles,
            &indices,
            context
                .shadow
                .batches
                .first()
                .and_then(|batch| batch.transmittance.as_ref())
                .map_or(&context.inputs.fallback.view, |attachment| {
                    &attachment.texture.default_view
                }),
        )),
    );
    Some(MoonlyNativeLocalBindGroup {
        bind_group,
        storage,
        tiles,
        indices,
    })
}

fn fill_native_uniforms(
    context: &SpotViewContext,
    projectors: &Query<(&MainEntity, &MoonlyProjector)>,
    scratch: &mut bins::NativeBuildScratch,
) {
    scratch.lights.clear();
    scratch.bounds.clear();
    let unshadowed = context
        .package
        .v_point
        .iter()
        .chain(&context.package.v_spot);
    for light in unshadowed.chain(&context.package.v_shadowed) {
        if !crate::visibility::light_visible(light, context.view) {
            continue;
        }
        let projector = find_projector(light, context.inputs, projectors);
        let sample = context.shadow.sample(light);
        scratch
            .bounds
            .push(bins::project_bounds(light, context.view));
        scratch.lights.push(MoonlySpotUniform::new(
            light,
            context.view,
            sample.as_ref(),
            projector.is_some(),
            &context.inputs.config.volumetrics,
        ));
    }
}

pub(crate) fn render_native_local(
    view: ViewQuery<(&MoonlyViewTargets, &MoonlyNativeLocalBindGroup), With<MoonlyCamera>>,
    cache: Res<PipelineCache>,
    pipeline: Res<MoonlyNativeLocalPipeline>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    mut context: RenderContext,
) {
    let (targets, group) = view.into_inner();
    let Some(pipeline) = cache.get_render_pipeline(pipeline.id) else {
        return;
    };
    let attachments = [Some(targets.accumulator.get_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_accum_native"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::AccumLocal),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_accum_native");
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &group.bind_group, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

#[cfg(test)]
mod tests {
    use super::*;
    use moonly_core::{MoonlyLight, MoonlyLightType};

    #[test]
    fn batch_requires_native_lights_without_virtual_gi() {
        let mut package = LightPackage::default();
        package.push(MoonlyLight {
            light_type: MoonlyLightType::Spot,
            flags: MoonlyLightFlags::NATIVE,
            ..MoonlyLight::default()
        });
        assert!(native_batch_eligible(&package, false));
        assert!(!native_batch_eligible(&package, true));
        package.push(MoonlyLight::default());
        assert!(!native_batch_eligible(&package, false));
        assert!(!native_batch_eligible(&LightPackage::default(), false));
    }
}

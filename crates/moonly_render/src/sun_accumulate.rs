use bevy::{
    asset::{AssetServer, load_embedded_asset},
    core_pipeline::FullscreenShader,
    ecs::system::SystemParam,
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            BlendComponent, BlendFactor, BlendOperation, BlendState, CachedRenderPipelineId,
            ColorTargetState, ColorWrites, CompareFunction, FilterMode, FragmentState,
            PipelineCache, RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
            SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType, TextureSampleType,
            UniformBuffer,
            binding_types::{sampler, texture_2d, texture_2d_array, texture_3d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        view::ExtractedView,
    },
};
use moonly_core::{LightPackage, MoonlyLight, MoonlySunCascade};

use crate::{
    camera::MoonlyCamera,
    material_lut::MoonlyMaterialLutTexture,
    projector::MoonlyFallbackProjector,
    shadow_targets::MoonlySpotShadowTarget,
    view_targets::{GBUFFER_FORMAT, MoonlyViewTargets},
};

#[derive(ShaderType)]
pub(crate) struct MoonlySunUniform {
    view_position: Vec4,
    view_forward: Vec4,
    direction_bias: Vec4,
    color: Vec4,
    splits_near: Vec4,
    splits_far: Vec4,
    shadow_near_from_world: Mat4,
    shadow_middle_from_world: Mat4,
    shadow_far_from_world: Mat4,
}

impl MoonlySunUniform {
    pub(crate) fn new(
        light: &MoonlyLight,
        view: &ExtractedView,
        cascades: &[MoonlySunCascade],
        target: &MoonlySpotShadowTarget,
        depth_bias: f32,
    ) -> Self {
        let split = |index: usize| cascades.get(index).map_or(0.0, |cascade| cascade.split_far);
        let shadow_texel = target
            .sun_batches
            .first()
            .and_then(|batch| batch.views.first())
            .map_or(0.0, |view| 1.0 / view.rect.size as f32);
        Self {
            view_position: view.world_from_view.translation().extend(shadow_texel),
            view_forward: view.world_from_view.forward().as_vec3().extend(0.0),
            direction_bias: light.direction.extend(depth_bias),
            color: light.color.extend(1.0),
            splits_near: Vec4::new(
                cascades.first().map_or(0.0, |cascade| cascade.split_near),
                split(0),
                split(1),
                split(2),
            ),
            splits_far: Vec4::new(
                split(0),
                split(1),
                split(2),
                if target.near_minmax.is_some() {
                    1.0
                } else {
                    0.0
                },
            ),
            shadow_near_from_world: cascade_matrix(target, 0),
            shadow_middle_from_world: cascade_matrix(target, 1),
            shadow_far_from_world: cascade_matrix(target, 2),
        }
    }
}

fn cascade_matrix(target: &MoonlySpotShadowTarget, index: usize) -> Mat4 {
    target
        .sun_batches
        .get(index)
        .and_then(|batch| batch.views.first())
        .map_or(Mat4::IDENTITY, |view| view.xform.combine)
}

#[derive(Resource)]
pub(crate) struct MoonlySunPipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedRenderPipelineId,
    sampler: Sampler,
}

#[derive(Component)]
pub(crate) struct MoonlySunBindGroup(BindGroup);

pub(crate) fn init_sun_pipeline(
    mut commands: Commands,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
) {
    let layout = sun_bind_group_layout();
    let shader = load_embedded_asset!(asset_server.as_ref(), "accum_direct.wesl");
    let id = pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("moonly_sun_accumulation".into()),
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
    let sampler = device.create_sampler(&SamplerDescriptor {
        compare: Some(CompareFunction::LessEqual),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    });
    commands.insert_resource(MoonlySunPipeline {
        layout,
        id,
        sampler,
    });
}

fn sun_bind_group_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "moonly_sun_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<MoonlySunUniform>(false),
                texture_3d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d(TextureSampleType::Depth),
                texture_2d(TextureSampleType::Depth),
                texture_2d(TextureSampleType::Depth),
                sampler(SamplerBindingType::Comparison),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    )
}

const fn additive_component() -> BlendComponent {
    BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    }
}

#[derive(SystemParam)]
pub(crate) struct SunPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    pipeline_cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlySunPipeline>,
    cascades: Res<'w, moonly_core::MoonlySunPackage>,
    lut: Res<'w, MoonlyMaterialLutTexture>,
    fallback: Res<'w, MoonlyFallbackProjector>,
    jitter: Res<'w, crate::jitter::MoonlyJitterTexture>,
}

pub(crate) fn prepare_sun_bind_groups(
    mut commands: Commands,
    inputs: SunPrepareInputs,
    views: Query<
        (
            Entity,
            &ExtractedView,
            &MoonlyViewTargets,
            &MoonlySpotShadowTarget,
            &crate::light_packages::MoonlyViewLightPackage,
        ),
        With<MoonlyCamera>,
    >,
) {
    for (entity, view, targets, shadow, package) in &views {
        let Some(bind_group) = create_sun_bind_group(&inputs, view, targets, shadow, &package.0)
        else {
            commands.entity(entity).remove::<MoonlySunBindGroup>();
            continue;
        };
        commands
            .entity(entity)
            .insert(MoonlySunBindGroup(bind_group));
    }
}

fn create_sun_bind_group(
    inputs: &SunPrepareInputs,
    view: &ExtractedView,
    targets: &MoonlyViewTargets,
    shadow: &MoonlySpotShadowTarget,
    package: &LightPackage,
) -> Option<BindGroup> {
    let light = package.v_direct.first()?;
    let near = shadow.sun_batches.first()?;
    let middle = shadow.sun_batches.get(1)?;
    let far = shadow.sun_batches.get(2)?;
    let minmax = shadow
        .near_minmax
        .as_ref()
        .map_or(&inputs.fallback.view, |target| &target.texture.default_view);
    let mut uniform = UniformBuffer::from(MoonlySunUniform::new(
        light,
        view,
        &inputs.cascades.cascades,
        shadow,
        inputs.cascades.depth_bias,
    ));
    uniform.write_buffer(&inputs.device, &inputs.queue);
    Some(
        inputs.device.create_bind_group(
            Some("moonly_sun_bind_group"),
            &inputs
                .pipeline_cache
                .get_bind_group_layout(&inputs.pipeline.layout),
            &BindGroupEntries::sequential((
                &targets.position.texture.default_view,
                &targets.normal.texture.default_view,
                &targets.albedo_gloss.texture.default_view,
                &uniform,
                &inputs.lut.view,
                &inputs.lut.sampler,
                &near.depth.texture.default_view,
                &middle.depth.texture.default_view,
                &far.depth.texture.default_view,
                &inputs.pipeline.sampler,
                minmax,
                &inputs.jitter.view,
                &inputs.jitter.sampler,
            )),
        ),
    )
}

pub(crate) fn render_sun_accumulation(
    view: ViewQuery<(&MoonlyViewTargets, &MoonlySunBindGroup), With<MoonlyCamera>>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<MoonlySunPipeline>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    mut context: RenderContext,
) {
    let (targets, bind_group) = view.into_inner();
    let Some(render_pipeline) = pipeline_cache.get_render_pipeline(pipeline.id) else {
        return;
    };
    let attachments = [Some(targets.accumulator.get_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_accum_direct"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::AccumDirect),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_accum_direct");
    pass.set_render_pipeline(render_pipeline);
    pass.set_bind_group(0, &bind_group.0, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

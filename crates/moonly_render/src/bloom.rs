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
            ColorTargetState, ColorWrites, FragmentState, PipelineCache, RenderPassDescriptor,
            RenderPipeline, RenderPipelineDescriptor, ShaderStages, ShaderType, TextureSampleType,
            UniformBuffer,
            binding_types::{texture_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        texture::ColorAttachment,
        view::ViewTarget,
    },
    shader::Shader,
};
use moonly_post::Hdr10PostConfig;

use crate::{camera::MoonlyCamera, view_targets::MoonlyViewTargets};

#[derive(Resource)]
pub(crate) struct MoonlyBloomPipeline {
    layout: BindGroupLayoutDescriptor,
    downsample: CachedRenderPipelineId,
    upsample: CachedRenderPipelineId,
}

#[derive(Component)]
pub(crate) struct ViewBloomBindGroups([BindGroup; 5]);

#[derive(ShaderType)]
struct MoonlyBloomUniform {
    input_size: Vec2,
    output_size: Vec2,
    mode: UVec4,
    params: Vec4,
}

pub(crate) fn init_bloom_pipeline(
    mut commands: Commands,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_bloom_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<MoonlyBloomUniform>(false),
            ),
        ),
    );
    let shader = load_embedded_asset!(asset_server.as_ref(), "bloom.wesl");
    let context = BloomPipelineContext {
        layout: &layout,
        fullscreen: &fullscreen,
        cache: &cache,
        shader: &shader,
    };
    let downsample = bloom_pipeline(context, "downsample", false);
    let upsample = bloom_pipeline(context, "upsample", true);
    commands.insert_resource(MoonlyBloomPipeline {
        layout,
        downsample,
        upsample,
    });
}

#[derive(Clone, Copy)]
struct BloomPipelineContext<'a> {
    layout: &'a BindGroupLayoutDescriptor,
    fullscreen: &'a FullscreenShader,
    cache: &'a PipelineCache,
    shader: &'a Handle<Shader>,
}

fn bloom_pipeline(
    context: BloomPipelineContext,
    entry: &'static str,
    additive: bool,
) -> CachedRenderPipelineId {
    let blend = additive.then(|| BlendState {
        color: additive_component(),
        alpha: additive_component(),
    });
    context
        .cache
        .queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("moonly_bloom".into()),
            layout: vec![context.layout.clone()],
            vertex: context.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: context.shader.clone(),
                entry_point: Some(entry.into()),
                targets: vec![Some(ColorTargetState {
                    format: crate::view_targets::GBUFFER_FORMAT,
                    blend,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        })
}

const fn additive_component() -> BlendComponent {
    BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    }
}

#[derive(SystemParam)]
pub(crate) struct BloomPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyBloomPipeline>,
    config: Res<'w, Hdr10PostConfig>,
}

type BloomView = (
    Entity,
    &'static crate::settings::MoonlyExposure,
    &'static MoonlyViewTargets,
    &'static ViewTarget,
    Option<&'static crate::MoonlyPostSettings>,
);

struct BloomPassSettings {
    stage: usize,
    exposure: f32,
    threshold: f32,
    softness: f32,
}

pub(crate) fn prepare_bloom_bind_groups(
    mut commands: Commands,
    inputs: BloomPrepareInputs,
    views: Query<BloomView, With<MoonlyCamera>>,
) {
    for (entity, camera, targets, view, settings) in &views {
        let settings = settings
            .copied()
            .unwrap_or_else(|| crate::MoonlyPostSettings::from_config(&inputs.config));
        let groups = [0_usize, 1, 2, 3, 4].map(|stage| {
            let source = bloom_source(targets, stage);
            let source_size = if stage == 0 {
                view.main_texture().size()
            } else {
                source.texture.texture.size()
            };
            let source_view = if stage == 0 {
                view.main_texture_view()
            } else {
                &source.texture.default_view
            };
            let pass = BloomPassSettings {
                stage,
                exposure: camera.0,
                threshold: settings.bloom_threshold,
                softness: settings.bloom_softness,
            };
            create_bloom_bind_group(
                &inputs,
                source_view,
                source_size,
                bloom_target(targets, stage),
                &pass,
            )
        });
        commands.entity(entity).insert(ViewBloomBindGroups(groups));
    }
}

fn bloom_source(targets: &MoonlyViewTargets, stage: usize) -> &ColorAttachment {
    if stage == 0 {
        return &targets.scene;
    }
    let source_level = if stage < 3 { stage - 1 } else { 5 - stage };
    targets.bloom.get(source_level).unwrap_or(&targets.scene)
}

fn bloom_target(targets: &MoonlyViewTargets, stage: usize) -> &ColorAttachment {
    let target_level = if stage < 3 { stage } else { 4 - stage };
    targets.bloom.get(target_level).unwrap_or(&targets.scene)
}

fn create_bloom_bind_group(
    inputs: &BloomPrepareInputs,
    source: &bevy::render::render_resource::TextureView,
    input: bevy::render::render_resource::Extent3d,
    target: &ColorAttachment,
    settings: &BloomPassSettings,
) -> BindGroup {
    let output = target.texture.texture.size();
    let uniform = MoonlyBloomUniform {
        input_size: Vec2::new(input.width as f32, input.height as f32),
        output_size: Vec2::new(output.width as f32, output.height as f32),
        mode: UVec4::splat(settings.stage as u32),
        params: Vec4::new(
            settings.exposure,
            settings.threshold,
            settings.softness,
            0.0,
        ),
    };
    let mut uniform = UniformBuffer::from(uniform);
    uniform.write_buffer(&inputs.device, &inputs.queue);
    inputs.device.create_bind_group(
        Some("moonly_bloom_bind_group"),
        &inputs.cache.get_bind_group_layout(&inputs.pipeline.layout),
        &BindGroupEntries::sequential((source, &uniform)),
    )
}

#[derive(SystemParam)]
pub(crate) struct BloomRenderInputs<'w> {
    cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyBloomPipeline>,
    config: Res<'w, Hdr10PostConfig>,
    timer: Res<'w, crate::gpu_timer::MoonlyGpuTimer>,
}

pub(crate) fn render_bloom(
    view: ViewQuery<
        (
            &MoonlyViewTargets,
            &ViewBloomBindGroups,
            Option<&crate::MoonlyPostSettings>,
        ),
        With<MoonlyCamera>,
    >,
    inputs: BloomRenderInputs,
    mut context: RenderContext,
) {
    let BloomRenderInputs {
        cache,
        pipeline,
        config,
        timer,
    } = inputs;
    let (targets, groups, settings) = view.into_inner();
    let intensity = settings.map_or(config.bloom_intensity, |settings| {
        settings.bloom_intensity
    });
    if intensity <= 0.0 {
        return;
    }
    for (stage, group) in groups.0.iter().enumerate() {
        let id = if stage < 3 {
            pipeline.downsample
        } else {
            pipeline.upsample
        };
        let Some(render_pipeline) = cache.get_render_pipeline(id) else {
            continue;
        };
        render_bloom_stage(
            bloom_target(targets, stage),
            group,
            render_pipeline,
            &timer,
            &mut context,
        );
    }
}

fn render_bloom_stage(
    target: &ColorAttachment,
    group: &BindGroup,
    pipeline: &RenderPipeline,
    timer: &crate::gpu_timer::MoonlyGpuTimer,
    context: &mut RenderContext,
) {
    let attachments = [Some(target.get_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_bloom"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::Bloom),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_bloom");
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

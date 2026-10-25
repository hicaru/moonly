use bevy::{
    asset::{AssetServer, load_embedded_asset},
    core_pipeline::FullscreenShader,
    ecs::system::SystemParam,
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, PipelineCache,
            RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages, ShaderType,
            TextureSampleType, UniformBuffer,
            binding_types::{texture_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        view::ExtractedView,
    },
};
use moonly_core::{AmbientOcclusionMode, AmbientOcclusionSettings, MoonlyConfig};

use crate::{
    camera::MoonlyCamera,
    view_targets::{GBUFFER_FORMAT, MoonlyViewTargets},
};

#[derive(Resource)]
pub(crate) struct MoonlyAoPipeline {
    layout: BindGroupLayoutDescriptor,
    pipeline: CachedRenderPipelineId,
}

#[derive(Component)]
pub(crate) struct ViewAoBindGroup(BindGroup);

#[derive(ShaderType)]
struct MoonlyAoUniform {
    clip_from_world: Mat4,
    view_from_world: Mat4,
    settings: Vec4,
    texel: Vec4,
}

impl MoonlyAoUniform {
    fn new(
        view: &ExtractedView,
        config: &MoonlyConfig,
        settings: &AmbientOcclusionSettings,
    ) -> Self {
        let mode = match config.ao_mode {
            AmbientOcclusionMode::Off => 0.0,
            AmbientOcclusionMode::Ssao => 1.0,
            AmbientOcclusionMode::Hdao => 2.0,
            AmbientOcclusionMode::ActivisionColoredAo => 3.0,
            AmbientOcclusionMode::EnhancedSsdo => 4.0,
        };
        Self {
            clip_from_world: view.clip_from_view * view.world_from_view.to_matrix().inverse(),
            view_from_world: view.world_from_view.to_matrix().inverse(),
            settings: Vec4::new(
                settings.reject_radius,
                settings.accept_radius,
                settings.intensity,
                mode,
            ),
            texel: Vec4::ZERO,
        }
    }
}

pub(crate) fn init_ao_pipeline(
    mut commands: Commands,
    fullscreen_shader: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_ao_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<MoonlyAoUniform>(false),
            ),
        ),
    );
    let descriptor = RenderPipelineDescriptor {
        label: Some("moonly_ao_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen_shader.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: load_embedded_asset!(asset_server.as_ref(), "ao.wesl"),
            targets: vec![Some(ColorTargetState {
                format: GBUFFER_FORMAT,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    };
    let pipeline = pipeline_cache.queue_render_pipeline(descriptor);
    commands.insert_resource(MoonlyAoPipeline { layout, pipeline });
}

#[derive(SystemParam)]
pub(crate) struct AoPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyAoPipeline>,
    config: Res<'w, MoonlyConfig>,
    settings: Res<'w, AmbientOcclusionSettings>,
}

pub(crate) fn prepare_ao_bind_groups(
    mut commands: Commands,
    inputs: AoPrepareInputs,
    views: Query<(Entity, &ExtractedView, &MoonlyViewTargets), With<MoonlyCamera>>,
) {
    for (entity, view, targets) in &views {
        let mut uniform =
            UniformBuffer::from(MoonlyAoUniform::new(view, &inputs.config, &inputs.settings));
        uniform.write_buffer(&inputs.device, &inputs.queue);
        let group = inputs.device.create_bind_group(
            Some("moonly_ao_bind_group"),
            &inputs.cache.get_bind_group_layout(&inputs.pipeline.layout),
            &BindGroupEntries::sequential((
                &targets.position.texture.default_view,
                &targets.normal.texture.default_view,
                &uniform,
            )),
        );
        commands.entity(entity).insert(ViewAoBindGroup(group));
    }
}

pub(crate) fn render_ao(
    view: ViewQuery<(&MoonlyViewTargets, &ViewAoBindGroup), With<MoonlyCamera>>,
    pipeline: Res<MoonlyAoPipeline>,
    pipeline_cache: Res<PipelineCache>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    mut context: RenderContext,
) {
    let (targets, group) = view.into_inner();
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline.pipeline) else {
        return;
    };
    let attachments = [Some(targets.ao.get_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_ao"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::AmbientOcclusion),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_ao");
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &group.0, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

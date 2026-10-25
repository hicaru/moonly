use bevy::{
    asset::{AssetServer, Handle, load_embedded_asset},
    core_pipeline::FullscreenShader,
    ecs::system::SystemParam,
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState,
            PipelineCache, RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages,
            ShaderType, SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat,
            TextureSampleType, UniformBuffer,
            binding_types::{texture_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        view::ViewTarget,
    },
    shader::Shader,
};
use moonly_post::{Hdr10PostConfig, LuminanceAdaptationState, OpticsMode};

use crate::{
    camera::{MoonlyCamera, MoonlyCameraRole, MoonlyLinearOutput},
    view_targets::MoonlyViewTargets,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MoonlyPostKey {
    format: TextureFormat,
    overlay: bool,
}

#[derive(Resource)]
pub(crate) struct MoonlyPostPipeline {
    layout: BindGroupLayoutDescriptor,
    fullscreen: FullscreenShader,
    shader: Handle<Shader>,
}

#[derive(Component)]
pub(crate) struct ViewPostPipeline(CachedRenderPipelineId);

#[derive(Component)]
pub(crate) struct ViewPostBindGroup(BindGroup);

#[derive(ShaderType)]
struct MoonlyPostUniform {
    exposure: Vec4,
    bloom: Vec4,
    modes: UVec4,
    display: Vec4,
}

impl MoonlyPostUniform {
    fn new(
        config: &Hdr10PostConfig,
        adaptation: &LuminanceAdaptationState,
        optics: OpticsMode,
        linear: bool,
    ) -> Self {
        Self {
            exposure: Vec4::new(
                config.exposure * adaptation.compute_exposure_scale(),
                config.contrast,
                config.vibrance,
                0.0,
            ),
            bloom: Vec4::new(config.bloom_intensity, config.flare_intensity, 0.0, 0.0),
            modes: UVec4::new(
                config.operator.shader_index(),
                optics.shader_index(),
                u32::from(linear),
                0,
            ),
            display: Vec4::new(config.max_nits, 0.0, 0.0, 0.0),
        }
    }
}

pub(crate) fn init_post_pipeline(
    mut commands: Commands,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_post_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<MoonlyPostUniform>(false),
            ),
        ),
    );
    commands.insert_resource(MoonlyPostPipeline {
        layout,
        fullscreen: fullscreen.clone(),
        shader: load_embedded_asset!(asset_server.as_ref(), "post.wesl"),
    });
}

impl SpecializedRenderPipeline for MoonlyPostPipeline {
    type Key = MoonlyPostKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("moonly_post_pipeline".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: key.overlay.then_some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        }
    }
}

pub(crate) fn prepare_post_pipelines(
    mut commands: Commands,
    cache: Res<PipelineCache>,
    pipeline: Res<MoonlyPostPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<MoonlyPostPipeline>>,
    views: Query<(Entity, &MoonlyCamera, &ViewTarget)>,
) {
    for (entity, camera, target) in &views {
        let Some(format) = target.out_texture_view_format() else {
            continue;
        };
        let key = MoonlyPostKey {
            format,
            overlay: camera.role == MoonlyCameraRole::Overlay,
        };
        let id = pipelines.specialize(&cache, &pipeline, key);
        commands.entity(entity).insert(ViewPostPipeline(id));
    }
}

#[derive(SystemParam)]
pub(crate) struct PostPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyPostPipeline>,
    config: Res<'w, Hdr10PostConfig>,
    adaptation: Res<'w, LuminanceAdaptationState>,
    optics: Res<'w, OpticsMode>,
}

pub(crate) fn prepare_post_bind_groups(
    mut commands: Commands,
    inputs: PostPrepareInputs,
    views: Query<
        (
            Entity,
            &MoonlyViewTargets,
            &ViewTarget,
            Option<&MoonlyLinearOutput>,
        ),
        With<MoonlyCamera>,
    >,
) {
    for (entity, targets, target, linear) in &views {
        let uniform = MoonlyPostUniform::new(
            &inputs.config,
            &inputs.adaptation,
            *inputs.optics,
            linear.is_some(),
        );
        let mut uniform = UniformBuffer::from(uniform);
        uniform.write_buffer(&inputs.device, &inputs.queue);
        let bloom = targets.bloom.first().unwrap_or(&targets.scene);
        let group = inputs.device.create_bind_group(
            Some("moonly_post_bind_group"),
            &inputs.cache.get_bind_group_layout(&inputs.pipeline.layout),
            &BindGroupEntries::sequential((
                target.main_texture_view(),
                &bloom.texture.default_view,
                &uniform,
            )),
        );
        commands.entity(entity).insert(ViewPostBindGroup(group));
    }
}

pub(crate) fn render_post(
    view: ViewQuery<(&ViewTarget, &ViewPostPipeline, &ViewPostBindGroup), With<MoonlyCamera>>,
    cache: Res<PipelineCache>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    mut context: RenderContext,
) {
    let (target, pipeline, group) = view.into_inner();
    let Some(pipeline) = cache.get_render_pipeline(pipeline.0) else {
        return;
    };
    let Some(attachment) = target.out_texture_color_attachment(None) else {
        return;
    };
    let attachments = [Some(attachment)];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_post"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::Post),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_post");
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &group.0, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

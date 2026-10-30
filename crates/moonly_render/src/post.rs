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
    },
    shader::Shader,
};
use moonly_post::{Hdr10PostConfig, OpticsMode};

use crate::{
    camera::{MoonlyCamera, MoonlyLinearOutput},
    view_targets::{GBUFFER_FORMAT, MoonlyViewTargets},
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
    grading: Vec4,
}

impl MoonlyPostUniform {
    fn new(
        settings: &crate::MoonlyPostSettings,
        optics: OpticsMode,
        linear: bool,
        camera_exposure: f32,
    ) -> Self {
        Self {
            exposure: Vec4::new(
                camera_exposure,
                settings.contrast,
                settings.saturation,
                settings.grading_exposure,
            ),
            bloom: Vec4::new(
                settings.bloom_intensity,
                settings.flare_intensity,
                settings.bloom_threshold,
                settings.bloom_softness,
            ),
            modes: UVec4::new(
                settings.operator.shader_index(),
                optics.shader_index(),
                u32::from(linear),
                u32::from(settings.exposure_mode != moonly_post::MoonlyExposureMode::Camera),
            ),
            display: Vec4::new(
                settings.max_nits,
                settings.vignette_intensity,
                settings.vignette_radius,
                settings.vignette_smoothness,
            ),
            grading: Vec4::new(
                settings.temperature,
                settings.tint,
                settings.chromatic_aberration,
                0.0,
            ),
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
    views: Query<Entity, With<MoonlyCamera>>,
) {
    for entity in &views {
        let key = MoonlyPostKey {
            format: GBUFFER_FORMAT,
            overlay: false,
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
    adaptation: Res<'w, moonly_post::LuminanceAdaptationState>,
    optics: Res<'w, OpticsMode>,
}

type PostView = (
    Entity,
    &'static crate::settings::MoonlyExposure,
    &'static MoonlyViewTargets,
    Option<&'static MoonlyLinearOutput>,
    Option<&'static crate::MoonlyPostSettings>,
);

pub(crate) fn prepare_post_bind_groups(
    mut commands: Commands,
    inputs: PostPrepareInputs,
    views: Query<PostView, With<MoonlyCamera>>,
) {
    for (entity, camera, targets, linear, settings) in &views {
        let settings = settings
            .copied()
            .unwrap_or_else(|| crate::MoonlyPostSettings::from_config(&inputs.config));
        let uniform = MoonlyPostUniform::new(
            &settings,
            *inputs.optics,
            linear.is_some(),
            settings.exposure(camera.0, &inputs.config, &inputs.adaptation),
        );
        let mut uniform = UniformBuffer::from(uniform);
        uniform.write_buffer(&inputs.device, &inputs.queue);
        let bloom = targets.bloom.first().unwrap_or(&targets.emissive);
        let group = inputs.device.create_bind_group(
            Some("moonly_post_bind_group"),
            &inputs.cache.get_bind_group_layout(&inputs.pipeline.layout),
            &BindGroupEntries::sequential((
                &targets.scene.texture.default_view,
                &bloom.texture.default_view,
                &uniform,
            )),
        );
        commands.entity(entity).insert(ViewPostBindGroup(group));
    }
}

pub(crate) fn render_post(
    view: ViewQuery<
        (
            &MoonlyViewTargets,
            &ViewPostPipeline,
            &ViewPostBindGroup,
            Option<&crate::MoonlyPostSettings>,
        ),
        With<MoonlyCamera>,
    >,
    cache: Res<PipelineCache>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    native_post: Res<moonly_core::MoonlyNativePost>,
    mut context: RenderContext,
) {
    let (targets, pipeline, group, settings) = view.into_inner();
    if native_post.0 || settings.is_some_and(|settings| settings.native_post) {
        return;
    }
    let Some(pipeline) = cache.get_render_pipeline(pipeline.0) else {
        return;
    };
    let attachments = [Some(targets.post_process.get_unsampled_attachment())];
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

use bevy::{
    asset::{AssetServer, Handle, load_embedded_asset},
    core_pipeline::FullscreenShader,
    ecs::system::SystemParam,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        diagnostic::RecordDiagnostics,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState, PipelineCache,
            RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages, ShaderType,
            SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat,
            TextureSampleType, UniformBuffer,
            binding_types::{texture_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        view::{ExtractedView, ViewTarget},
    },
    shader::{Shader, ShaderDefVal},
};
use moonly_core::{
    AmbientOcclusionMode, AmbientOcclusionSettings, MoonlyConfig, WeatherEnvironment,
};

use crate::{
    camera::{MoonlyCamera, MoonlyCameraRole, MoonlyDebugView},
    shadow_targets::MoonlySpotShadowTarget,
    view_targets::MoonlyViewTargets,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MoonlyCombineKey {
    format: TextureFormat,
    debug_view: MoonlyDebugView,
    overlay: bool,
}

#[derive(Resource)]
pub(crate) struct MoonlyCombinePipeline {
    layout: BindGroupLayoutDescriptor,
    fullscreen_shader: FullscreenShader,
    shader: Handle<Shader>,
}

#[derive(Component)]
pub(crate) struct ViewCombinePipeline(CachedRenderPipelineId);

#[derive(Component)]
pub(crate) struct ViewCombineBindGroup(BindGroup);

#[derive(ShaderType)]
struct MoonlyEnvironmentUniform {
    ambient: Vec4,
    hemi: Vec4,
    fog_color: Vec4,
    fog_plane: Vec4,
    ao: Vec4,
    view_position: Vec4,
    fog_params: Vec4,
}

impl MoonlyEnvironmentUniform {
    fn new(
        environment: &WeatherEnvironment,
        clip_from_world: Mat4,
        config: &MoonlyConfig,
        settings: &AmbientOcclusionSettings,
        view_position: Vec3,
    ) -> Self {
        let mode = match config.ao_mode {
            AmbientOcclusionMode::Off => 0.0,
            AmbientOcclusionMode::Ssao => 1.0,
            AmbientOcclusionMode::Hdao => 2.0,
            AmbientOcclusionMode::ActivisionColoredAo => 3.0,
            AmbientOcclusionMode::EnhancedSsdo => 4.0,
        };
        Self {
            ambient: environment.ambient_color.extend(1.0),
            hemi: environment.hemi_color,
            fog_color: environment.fog_color.extend(1.0),
            fog_plane: environment.fog_plane_for_view(clip_from_world),
            ao: Vec4::new(mode, settings.ssdo_bounce_strength, 0.0, 0.0),
            view_position: view_position.extend(1.0),
            fog_params: Vec4::new(
                environment.fog_density,
                environment.fog_height_falloff,
                environment.fog_height_reference,
                0.0,
            ),
        }
    }
}

#[derive(SystemParam)]
pub(crate) struct CombinePrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    pipeline_cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyCombinePipeline>,
    environment: Res<'w, WeatherEnvironment>,
    config: Res<'w, MoonlyConfig>,
    ao_settings: Res<'w, AmbientOcclusionSettings>,
}

pub(crate) fn init_combine_pipeline(
    mut commands: Commands,
    fullscreen_shader: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_combine_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Depth),
                uniform_buffer::<MoonlyEnvironmentUniform>(false),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    commands.insert_resource(MoonlyCombinePipeline {
        layout,
        fullscreen_shader: fullscreen_shader.clone(),
        shader: load_embedded_asset!(asset_server.as_ref(), "combine.wesl"),
    });
}

impl SpecializedRenderPipeline for MoonlyCombinePipeline {
    type Key = MoonlyCombineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("moonly_combine_pipeline".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen_shader.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: vec![
                    ShaderDefVal::UInt("MOONLY_DEBUG_VIEW".into(), key.debug_view.index()),
                    ShaderDefVal::UInt("MOONLY_OVERLAY".into(), u32::from(key.overlay)),
                ],
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        }
    }
}

pub(crate) fn prepare_combine_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<MoonlyCombinePipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<MoonlyCombinePipeline>>,
    views: Query<(Entity, &MoonlyCamera, &MoonlyDebugView)>,
) {
    for (entity, camera, debug_view) in &views {
        let key = MoonlyCombineKey {
            format: crate::view_targets::GBUFFER_FORMAT,
            debug_view: *debug_view,
            overlay: camera.role == MoonlyCameraRole::Overlay,
        };
        let id = pipelines.specialize(&pipeline_cache, &pipeline, key);
        commands.entity(entity).insert(ViewCombinePipeline(id));
    }
}

type CombineViewQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ExtractedView,
        &'static MoonlyViewTargets,
        &'static MoonlySpotShadowTarget,
    ),
    With<MoonlyCamera>,
>;

pub(crate) fn prepare_combine_bind_groups(
    mut commands: Commands,
    inputs: CombinePrepareInputs,
    views: CombineViewQuery,
) {
    for (entity, view, targets, shadow) in &views {
        let Some(shadow_depth) = shadow.first_depth() else {
            continue;
        };
        let depth = targets
            .depth
            .depth_stencil_views()
            .depth_only_view()
            .unwrap_or(&targets.depth.texture.default_view);
        let clip_from_world = view.clip_from_view * view.world_from_view.to_matrix().inverse();
        let mut environment = UniformBuffer::from(MoonlyEnvironmentUniform::new(
            &inputs.environment,
            clip_from_world,
            &inputs.config,
            &inputs.ao_settings,
            view.world_from_view.translation(),
        ));
        environment.write_buffer(&inputs.device, &inputs.queue);
        let bind_group = inputs.device.create_bind_group(
            Some("moonly_combine_bind_group"),
            &inputs
                .pipeline_cache
                .get_bind_group_layout(&inputs.pipeline.layout),
            &BindGroupEntries::sequential((
                &targets.position.texture.default_view,
                &targets.normal.texture.default_view,
                &targets.albedo_gloss.texture.default_view,
                &targets.accumulator.texture.default_view,
                depth,
                shadow_depth,
                &environment,
                &targets.ao.texture.default_view,
                &targets.volumetrics.texture.default_view,
                &targets.local_volumetrics.texture.default_view,
                &targets.froxel.texture.default_view,
            )),
        );
        commands
            .entity(entity)
            .insert(ViewCombineBindGroup(bind_group));
    }
}

pub(crate) fn render_combine(
    view: ViewQuery<
        (
            &ExtractedCamera,
            &ViewTarget,
            &ViewCombinePipeline,
            &ViewCombineBindGroup,
        ),
        With<MoonlyCamera>,
    >,
    pipeline_cache: Res<PipelineCache>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    mut context: RenderContext,
) {
    let (camera, target, pipeline_id, bind_group) = view.into_inner();
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_id.0) else {
        return;
    };
    let attachments = [Some(target.get_unsampled_color_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_combine"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::Combine),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_combine");
    if let Some(viewport) = &camera.viewport {
        pass.set_camera_viewport(viewport);
    }
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &bind_group.0, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

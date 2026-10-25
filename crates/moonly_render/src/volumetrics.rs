use bevy::{
    asset::{AssetServer, load_embedded_asset},
    core_pipeline::FullscreenShader,
    ecs::system::SystemParam,
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, CompareFunction, FilterMode,
            FragmentState, PipelineCache, RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
            SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType, TextureSampleType,
            UniformBuffer,
            binding_types::{sampler, texture_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        view::ExtractedView,
    },
};
use moonly_core::{LightPackage, MoonlySunPackage};
use moonly_volumetrics::{SunShaftsConfig, SunShaftsMode};

use crate::{
    camera::MoonlyCamera,
    shadow_targets::MoonlySpotShadowTarget,
    sun_accumulate::MoonlySunUniform,
    view_targets::{GBUFFER_FORMAT, MoonlyViewTargets},
};

#[derive(Resource)]
pub(crate) struct MoonlyVolumetricPipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedRenderPipelineId,
    sampler: Sampler,
}

#[derive(Component)]
pub(crate) struct ViewVolumetricBindGroup(BindGroup);

#[derive(ShaderType)]
struct MoonlyVolumetricUniform {
    sun: MoonlySunUniform,
    world_from_clip: Mat4,
    params: Vec4,
    screen: Vec4,
}

pub(crate) fn init_volumetric_pipeline(
    mut commands: Commands,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_volumetric_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<MoonlyVolumetricUniform>(false),
                texture_2d(TextureSampleType::Depth),
                texture_2d(TextureSampleType::Depth),
                texture_2d(TextureSampleType::Depth),
                sampler(SamplerBindingType::Comparison),
            ),
        ),
    );
    let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("moonly_volumetrics".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: load_embedded_asset!(asset_server.as_ref(), "volumetrics.wesl"),
            targets: vec![Some(ColorTargetState {
                format: GBUFFER_FORMAT,
                blend: None,
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
    commands.insert_resource(MoonlyVolumetricPipeline {
        layout,
        id,
        sampler,
    });
}

#[derive(SystemParam)]
pub(crate) struct VolumetricPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyVolumetricPipeline>,
    cascades: Res<'w, MoonlySunPackage>,
    config: Res<'w, SunShaftsConfig>,
}

pub(crate) fn prepare_volumetric_bind_groups(
    mut commands: Commands,
    inputs: VolumetricPrepareInputs,
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
        let Some(group) = create_volumetric_bind_group(&inputs, view, targets, shadow, &package.0)
        else {
            commands.entity(entity).remove::<ViewVolumetricBindGroup>();
            continue;
        };
        commands
            .entity(entity)
            .insert(ViewVolumetricBindGroup(group));
    }
}

fn create_volumetric_bind_group(
    inputs: &VolumetricPrepareInputs,
    view: &ExtractedView,
    targets: &MoonlyViewTargets,
    shadow: &MoonlySpotShadowTarget,
    package: &LightPackage,
) -> Option<BindGroup> {
    if matches!(inputs.config.mode, SunShaftsMode::Off) {
        return None;
    }
    let light = package.v_direct.first()?;
    let near = shadow.sun_batches.first()?;
    let middle = shadow.sun_batches.get(1)?;
    let far = shadow.sun_batches.get(2)?;
    let clip_from_world = view.clip_from_view * view.world_from_view.to_matrix().inverse();
    let mode = match inputs.config.mode {
        SunShaftsMode::Off => 0.0,
        SunShaftsMode::Volumetric => 1.0,
        SunShaftsMode::ScreenSpace => 2.0,
        SunShaftsMode::Combined => 3.0,
    };
    let sun_position = view.world_from_view.translation() - light.direction * 1000.0;
    let sun_clip = clip_from_world * sun_position.extend(1.0);
    let sun_ndc = sun_clip.xy() / sun_clip.w.max(1.0e-6);
    let screen = Vec4::new(
        sun_ndc.x * 0.5 + 0.5,
        sun_ndc.y * -0.5 + 0.5,
        inputs.config.screen_space_decay,
        f32::from(sun_clip.w > 0.0),
    );
    let uniform = MoonlyVolumetricUniform {
        sun: MoonlySunUniform::new(
            light,
            view,
            &inputs.cascades.cascades,
            shadow,
            inputs.cascades.depth_bias,
        ),
        world_from_clip: clip_from_world.inverse(),
        params: Vec4::new(
            inputs.config.density,
            inputs.config.raymarch_steps as f32,
            inputs.config.mie_anisotropy,
            mode,
        ),
        screen,
    };
    let mut uniform = UniformBuffer::from(uniform);
    uniform.write_buffer(&inputs.device, &inputs.queue);
    Some(inputs.device.create_bind_group(
        Some("moonly_volumetric_bind_group"),
        &inputs.cache.get_bind_group_layout(&inputs.pipeline.layout),
        &BindGroupEntries::sequential((
            &targets.position.texture.default_view,
            &uniform,
            &near.depth.texture.default_view,
            &middle.depth.texture.default_view,
            &far.depth.texture.default_view,
            &inputs.pipeline.sampler,
        )),
    ))
}

pub(crate) fn render_volumetrics(
    view: ViewQuery<(&MoonlyViewTargets, &ViewVolumetricBindGroup), With<MoonlyCamera>>,
    cache: Res<PipelineCache>,
    pipeline: Res<MoonlyVolumetricPipeline>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    mut context: RenderContext,
) {
    let (targets, group) = view.into_inner();
    let Some(render_pipeline) = cache.get_render_pipeline(pipeline.id) else {
        return;
    };
    let attachments = [Some(targets.volumetrics.get_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_volumetrics"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::Volumetrics),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_volumetrics");
    pass.set_render_pipeline(render_pipeline);
    pass.set_bind_group(0, &group.0, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

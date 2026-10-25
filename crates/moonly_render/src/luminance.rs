use std::sync::atomic::{AtomicU32, Ordering};

use bevy::{
    asset::{AssetServer, load_embedded_asset},
    color::LinearRgba,
    core_pipeline::FullscreenShader,
    ecs::system::SystemParam,
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, Extent3d, FragmentState,
            PipelineCache, RenderPassDescriptor, RenderPipelineDescriptor, ShaderStages,
            ShaderType, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
            TextureUsages, UniformBuffer,
            binding_types::{texture_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        texture::{ColorAttachment, TextureCache},
    },
};

use crate::{
    camera::MoonlyCamera,
    luminance_readback::{
        LuminanceReadback, LuminanceResultSender, create_readback, schedule_readback,
    },
    view_targets::MoonlyViewTargets,
};

const LUMINANCE_SIZES: [u32; 4] = [256, 64, 8, 1];
const LUMINANCE_UPDATE_INTERVAL: u32 = 4;

#[derive(Resource)]
pub(crate) struct MoonlyLuminancePipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedRenderPipelineId,
    frame: AtomicU32,
}

#[derive(Component)]
pub(crate) struct ViewLuminanceTargets {
    targets: [ColorAttachment; 4],
    readback: LuminanceReadback,
}

#[derive(Component)]
pub(crate) struct ViewLuminanceBindGroups([BindGroup; 4]);

#[derive(ShaderType)]
struct MoonlyLuminanceUniform {
    input_size: Vec2,
    output_size: Vec2,
    mode: UVec4,
}

pub(crate) fn init_luminance_pipeline(
    mut commands: Commands,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_luminance_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<MoonlyLuminanceUniform>(false),
            ),
        ),
    );
    let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("moonly_luminance".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: load_embedded_asset!(asset_server.as_ref(), "luminance.wesl"),
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::R32Float,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(MoonlyLuminancePipeline {
        layout,
        id,
        frame: AtomicU32::new(0),
    });
}

pub(crate) fn prepare_luminance_targets(
    mut commands: Commands,
    device: Res<RenderDevice>,
    sender: Res<LuminanceResultSender>,
    mut cache: ResMut<TextureCache>,
    views: Query<(Entity, Option<&ViewLuminanceTargets>), With<MoonlyCamera>>,
) {
    for (entity, existing) in &views {
        if existing.is_some() {
            continue;
        }
        let targets = LUMINANCE_SIZES.map(|size| luminance_target(&device, &mut cache, size));
        commands.entity(entity).insert(ViewLuminanceTargets {
            targets,
            readback: create_readback(&device, &sender),
        });
    }
}

fn luminance_target(device: &RenderDevice, cache: &mut TextureCache, size: u32) -> ColorAttachment {
    let texture = cache.get(
        device,
        TextureDescriptor {
            label: Some("moonly_luminance_target"),
            size: Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::R32Float,
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        },
    );
    ColorAttachment::new(texture, None, None, Some(LinearRgba::NONE.into()))
}

#[derive(SystemParam)]
pub(crate) struct LuminancePrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyLuminancePipeline>,
}

pub(crate) fn prepare_luminance_bind_groups(
    mut commands: Commands,
    inputs: LuminancePrepareInputs,
    views: Query<(Entity, &MoonlyViewTargets, &ViewLuminanceTargets), With<MoonlyCamera>>,
) {
    for (entity, scene, luminance) in &views {
        let groups = [0_usize, 1, 2, 3].map(|stage| {
            let source_size = if stage == 0 {
                scene.scene.texture.texture.size()
            } else {
                luminance_target_size(stage - 1)
            };
            let source = if stage == 0 {
                &scene.scene.texture.default_view
            } else {
                luminance
                    .targets
                    .get(stage - 1)
                    .map_or(&scene.scene.texture.default_view, |target| {
                        &target.texture.default_view
                    })
            };
            create_luminance_bind_group(&inputs, source, source_size, stage)
        });
        commands
            .entity(entity)
            .insert(ViewLuminanceBindGroups(groups));
    }
}

fn luminance_target_size(stage: usize) -> Extent3d {
    let size = LUMINANCE_SIZES.get(stage).copied().unwrap_or(1);
    Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    }
}

fn create_luminance_bind_group(
    inputs: &LuminancePrepareInputs,
    source: &bevy::render::render_resource::TextureView,
    source_size: Extent3d,
    stage: usize,
) -> BindGroup {
    let output_size = LUMINANCE_SIZES.get(stage).copied().unwrap_or(1) as f32;
    let uniform = MoonlyLuminanceUniform {
        input_size: Vec2::new(source_size.width as f32, source_size.height as f32),
        output_size: Vec2::splat(output_size),
        mode: UVec4::splat(stage as u32),
    };
    let mut uniform = UniformBuffer::from(uniform);
    uniform.write_buffer(&inputs.device, &inputs.queue);
    inputs.device.create_bind_group(
        Some("moonly_luminance_bind_group"),
        &inputs.cache.get_bind_group_layout(&inputs.pipeline.layout),
        &BindGroupEntries::sequential((source, &uniform)),
    )
}

pub(crate) fn render_luminance(
    view: ViewQuery<(&ViewLuminanceTargets, &ViewLuminanceBindGroups), With<MoonlyCamera>>,
    cache: Res<PipelineCache>,
    pipeline: Res<MoonlyLuminancePipeline>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    mut context: RenderContext,
) {
    if !pipeline
        .frame
        .fetch_add(1, Ordering::Relaxed)
        .is_multiple_of(LUMINANCE_UPDATE_INTERVAL)
    {
        return;
    }
    let (targets, groups) = view.into_inner();
    let Some(render_pipeline) = cache.get_render_pipeline(pipeline.id) else {
        return;
    };
    trace!("Moonly luminance reduction");
    for (target, group) in targets.targets.iter().zip(groups.0.iter()) {
        render_luminance_stage(target, group, render_pipeline, &timer, &mut context);
    }
    if let Some(target) = targets.targets.last() {
        schedule_readback(&targets.readback, context.command_encoder(), target);
    }
}

fn render_luminance_stage(
    target: &ColorAttachment,
    group: &BindGroup,
    pipeline: &bevy::render::render_resource::RenderPipeline,
    timer: &crate::gpu_timer::MoonlyGpuTimer,
    context: &mut RenderContext,
) {
    let attachments = [Some(target.get_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let timestamp_writes = timer.timestamp_writes(crate::diagnostics::MoonlyPass::Luminance);
    trace!(
        timestamped = timestamp_writes.is_some(),
        "Moonly luminance timestamp"
    );
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_luminance_reduce"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_luminance_reduce");
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, group, &[]);
    pass.draw(0..3, 0..1);
    span.end(&mut pass);
}

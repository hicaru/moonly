use bevy::{
    asset::{AssetServer, load_embedded_asset},
    core_pipeline::FullscreenShader,
    ecs::system::SystemParam,
    prelude::*,
    render::{
        diagnostic::RecordDiagnostics,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, ColorTargetState, ColorWrites, CompareFunction, Extent3d,
            FilterMode, FragmentState, PipelineCache, RenderPassDescriptor,
            RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
            ShaderType, TexelCopyBufferLayout, TextureDescriptor, TextureDimension, TextureFormat,
            TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, UniformBuffer,
            binding_types::{sampler, texture_2d, texture_3d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        view::ExtractedView,
    },
};
use moonly_core::{LightPackage, MoonlySunPackage};
use moonly_volumetrics::{FROXEL_GRID_SIZE, MoonlyFroxelConfig};

use crate::{
    camera::MoonlyCamera,
    shadow_targets::MoonlySpotShadowTarget,
    sun_accumulate::MoonlySunUniform,
    view_targets::{GBUFFER_FORMAT, MoonlyViewTargets},
};

#[derive(Resource)]
pub(crate) struct MoonlyFroxelTexture(pub TextureView);

#[derive(Resource)]
pub(crate) struct MoonlyFroxelPipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedRenderPipelineId,
    sampler: Sampler,
}

#[derive(Component)]
pub(crate) struct ViewFroxelBindGroup(BindGroup);

#[derive(ShaderType)]
struct MoonlyFroxelUniform {
    sun: MoonlySunUniform,
    world_from_clip: Mat4,
    grid_origin: Vec4,
    grid_extent: Vec4,
    params: Vec4,
}

pub(crate) fn init_froxel_texture(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let mut data = Vec::new();
    for z in 0..FROXEL_GRID_SIZE.z {
        for y in 0..FROXEL_GRID_SIZE.y {
            for x in 0..FROXEL_GRID_SIZE.x {
                let density = if (x + y + z) % 4 == 0 { 1.5 } else { 0.5 };
                for value in [0.36_f32, 0.42, 0.50, density] {
                    data.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
    }
    let size = Extent3d {
        width: FROXEL_GRID_SIZE.x,
        height: FROXEL_GRID_SIZE.y,
        depth_or_array_layers: FROXEL_GRID_SIZE.z,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("moonly_froxel_density"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D3,
        format: TextureFormat::Rgba32Float,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &data,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(FROXEL_GRID_SIZE.x * 16),
            rows_per_image: Some(FROXEL_GRID_SIZE.y),
        },
        size,
    );
    commands.insert_resource(MoonlyFroxelTexture(
        texture.create_view(&TextureViewDescriptor::default()),
    ));
}

pub(crate) fn init_froxel_pipeline(
    mut commands: Commands,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_froxel_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<MoonlyFroxelUniform>(false),
                texture_2d(TextureSampleType::Depth),
                texture_2d(TextureSampleType::Depth),
                texture_2d(TextureSampleType::Depth),
                sampler(SamplerBindingType::Comparison),
                texture_3d(TextureSampleType::Float { filterable: false }),
            ),
        ),
    );
    let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("moonly_froxel".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: load_embedded_asset!(asset_server.as_ref(), "froxel.wesl"),
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
    commands.insert_resource(MoonlyFroxelPipeline {
        layout,
        id,
        sampler,
    });
}

#[derive(SystemParam)]
pub(crate) struct FroxelPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyFroxelPipeline>,
    package: Res<'w, LightPackage>,
    cascades: Res<'w, MoonlySunPackage>,
    config: Res<'w, MoonlyFroxelConfig>,
    texture: Res<'w, MoonlyFroxelTexture>,
}

pub(crate) fn prepare_froxel_bind_groups(
    mut commands: Commands,
    inputs: FroxelPrepareInputs,
    views: Query<
        (
            Entity,
            &ExtractedView,
            &MoonlyViewTargets,
            &MoonlySpotShadowTarget,
        ),
        With<MoonlyCamera>,
    >,
) {
    for (entity, view, targets, shadow) in &views {
        let Some(group) = create_froxel_bind_group(&inputs, view, targets, shadow) else {
            commands.entity(entity).remove::<ViewFroxelBindGroup>();
            continue;
        };
        commands.entity(entity).insert(ViewFroxelBindGroup(group));
    }
}

fn create_froxel_bind_group(
    inputs: &FroxelPrepareInputs,
    view: &ExtractedView,
    targets: &MoonlyViewTargets,
    shadow: &MoonlySpotShadowTarget,
) -> Option<BindGroup> {
    if !inputs.config.enabled {
        return None;
    }
    let light = inputs.package.v_direct.first()?;
    let near = shadow.sun_batches.first()?;
    let middle = shadow.sun_batches.get(1)?;
    let far = shadow.sun_batches.get(2)?;
    let clip_from_world = view.clip_from_view * view.world_from_view.to_matrix().inverse();
    let uniform = MoonlyFroxelUniform {
        sun: MoonlySunUniform::new(
            light,
            view,
            &inputs.cascades.cascades,
            shadow,
            inputs.cascades.depth_bias,
        ),
        world_from_clip: clip_from_world.inverse(),
        grid_origin: inputs.config.grid_origin.extend(0.0),
        grid_extent: inputs.config.grid_extent.extend(0.0),
        params: Vec4::new(inputs.config.density, inputs.config.steps as f32, 0.0, 0.0),
    };
    let mut uniform = UniformBuffer::from(uniform);
    uniform.write_buffer(&inputs.device, &inputs.queue);
    Some(inputs.device.create_bind_group(
        Some("moonly_froxel_bind_group"),
        &inputs.cache.get_bind_group_layout(&inputs.pipeline.layout),
        &BindGroupEntries::sequential((
            &targets.position.texture.default_view,
            &uniform,
            &near.depth.texture.default_view,
            &middle.depth.texture.default_view,
            &far.depth.texture.default_view,
            &inputs.pipeline.sampler,
            &inputs.texture.0,
        )),
    ))
}

type FroxelView<'w, 's> = ViewQuery<
    'w,
    's,
    (
        &'static MoonlyViewTargets,
        Option<&'static ViewFroxelBindGroup>,
    ),
    (With<MoonlyCamera>, Without<crate::MoonlyExternalFog>),
>;

pub(crate) fn render_froxel(
    view: FroxelView,
    cache: Res<PipelineCache>,
    pipeline: Res<MoonlyFroxelPipeline>,
    timer: Res<crate::gpu_timer::MoonlyGpuTimer>,
    mut context: RenderContext,
) {
    let (targets, group) = view.into_inner();
    let Some(render_pipeline) = cache.get_render_pipeline(pipeline.id) else {
        return;
    };
    let attachments = [Some(targets.froxel.get_attachment())];
    let diagnostics = context.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_froxel"),
        color_attachments: &attachments,
        depth_stencil_attachment: None,
        timestamp_writes: timer.timestamp_writes(crate::diagnostics::MoonlyPass::Volumetrics),
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let span = diagnostics.pass_span(&mut pass, "moonly_froxel");
    if let Some(group) = group {
        pass.set_render_pipeline(render_pipeline);
        pass.set_bind_group(0, &group.0, &[]);
        pass.draw(0..3, 0..1);
    }
    span.end(&mut pass);
}

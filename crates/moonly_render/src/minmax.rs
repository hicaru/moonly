use bevy::{
    asset::{AssetServer, load_embedded_asset},
    prelude::*,
    render::{
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor, Extent3d,
            PipelineCache, ShaderStages, ShaderType, StorageTextureAccess, TextureDescriptor,
            TextureDimension, TextureFormat, TextureSampleType, TextureUsages, UniformBuffer,
            binding_types::{texture_2d, texture_storage_2d, uniform_buffer},
        },
        renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery},
        texture::CachedTexture,
    },
};

use crate::{camera::MoonlyCamera, shadow_targets::MoonlySpotShadowTarget};

#[derive(ShaderType)]
struct MinMaxSettings {
    source_size: u32,
}

#[derive(Resource)]
pub(crate) struct MoonlyMinMaxPipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedComputePipelineId,
}

pub(crate) struct MoonlyMinMaxTarget {
    pub texture: CachedTexture,
    pub size: u32,
    pub bind_group: Option<BindGroup>,
}

pub(crate) fn init_minmax_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_minmax_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                texture_2d(TextureSampleType::Depth),
                texture_storage_2d(TextureFormat::Rg32Float, StorageTextureAccess::WriteOnly),
                uniform_buffer::<MinMaxSettings>(false),
            ),
        ),
    );
    let id = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some("moonly_minmax_4x4".into()),
        layout: vec![layout.clone()],
        shader: load_embedded_asset!(asset_server.as_ref(), "minmax.wesl"),
        ..default()
    });
    commands.insert_resource(MoonlyMinMaxPipeline { layout, id });
}

pub(crate) fn prepare_minmax_bind_groups(
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<MoonlyMinMaxPipeline>,
    mut views: Query<&mut MoonlySpotShadowTarget, With<MoonlyCamera>>,
) {
    for mut target in &mut views {
        let Some(depth) = target
            .sun_batches
            .first()
            .map(|batch| batch.depth.texture.default_view.clone())
        else {
            continue;
        };
        let Some(minmax) = target.near_minmax.as_mut() else {
            continue;
        };
        let mut settings = UniformBuffer::from(MinMaxSettings {
            source_size: minmax.size * 4,
        });
        settings.write_buffer(&device, &queue);
        minmax.bind_group = Some(device.create_bind_group(
            Some("moonly_minmax_bind_group"),
            &pipeline_cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((&depth, &minmax.texture.default_view, &settings)),
        ));
    }
}

pub(crate) fn render_sun_minmax(
    view: ViewQuery<&MoonlySpotShadowTarget, With<MoonlyCamera>>,
    pipeline_cache: Res<PipelineCache>,
    pipeline: Res<MoonlyMinMaxPipeline>,
    mut context: RenderContext,
) {
    let Some(minmax) = view.into_inner().near_minmax.as_ref() else {
        return;
    };
    let (Some(compute_pipeline), Some(bind_group)) = (
        pipeline_cache.get_compute_pipeline(pipeline.id),
        minmax.bind_group.as_ref(),
    ) else {
        return;
    };
    let mut pass = context
        .command_encoder()
        .begin_compute_pass(&ComputePassDescriptor {
            label: Some("moonly_minmax_4x4"),
            timestamp_writes: None,
        });
    pass.set_pipeline(compute_pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    let groups = minmax.size.div_ceil(8);
    pass.dispatch_workgroups(groups, groups, 1);
}

pub(crate) fn minmax_descriptor(size: u32) -> TextureDescriptor<'static> {
    TextureDescriptor {
        label: Some("moonly_minmax_near"),
        size: Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rg32Float,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }
}

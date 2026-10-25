use bevy::{
    asset::{AssetServer, load_embedded_asset},
    core_pipeline::FullscreenShader,
    prelude::*,
    render::{
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            CachedRenderPipelineId, CompareFunction, DepthBiasState, DepthStencilState,
            FragmentState, PipelineCache, RenderPassDescriptor, RenderPipelineDescriptor,
            ShaderStages, StencilState, StoreOp, binding_types::texture_depth_2d,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::ViewDepthStencilTexture,
    },
};

use crate::{camera::MoonlyCamera, view_targets::MoonlyViewTargets};

#[derive(Resource)]
pub(crate) struct MoonlyDepthCopyPipeline {
    layout: BindGroupLayoutDescriptor,
    id: CachedRenderPipelineId,
}

#[derive(Component)]
pub(crate) struct ViewDepthCopyBindGroup(BindGroup);

pub(crate) fn init_depth_copy_pipeline(
    mut commands: Commands,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_depth_copy_layout",
        &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, texture_depth_2d()),
    );
    let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("moonly_depth_copy".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: load_embedded_asset!(asset_server.as_ref(), "depth_copy.wesl"),
            targets: vec![],
            ..default()
        }),
        depth_stencil: Some(DepthStencilState {
            format: bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(CompareFunction::Always),
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        }),
        ..default()
    });
    commands.insert_resource(MoonlyDepthCopyPipeline { layout, id });
}

pub(crate) fn prepare_depth_copy_bind_groups(
    mut commands: Commands,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
    pipeline: Res<MoonlyDepthCopyPipeline>,
    views: Query<(Entity, &MoonlyViewTargets), With<MoonlyCamera>>,
) {
    for (entity, targets) in &views {
        let Some(depth) = targets.depth.depth_stencil_views().depth_only_view() else {
            continue;
        };
        let group = device.create_bind_group(
            Some("moonly_depth_copy_bind_group"),
            &cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::single(depth),
        );
        commands
            .entity(entity)
            .insert(ViewDepthCopyBindGroup(group));
    }
}

pub(crate) fn render_depth_copy(
    view: ViewQuery<(&ViewDepthStencilTexture, &ViewDepthCopyBindGroup), With<MoonlyCamera>>,
    cache: Res<PipelineCache>,
    pipeline: Res<MoonlyDepthCopyPipeline>,
    mut context: RenderContext,
) {
    let (depth, group) = view.into_inner();
    let Some(render_pipeline) = cache.get_render_pipeline(pipeline.id) else {
        return;
    };
    let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("moonly_depth_copy"),
        color_attachments: &[],
        depth_stencil_attachment: Some(depth.get_attachment(StoreOp::Store)),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(render_pipeline);
    pass.set_bind_group(0, &group.0, &[]);
    pass.draw(0..3, 0..1);
}

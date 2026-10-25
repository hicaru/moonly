use bevy::{
    asset::{AssetServer, load_embedded_asset},
    mesh::VertexBufferLayout,
    prelude::*,
    render::{
        render_resource::{
            BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendComponent, BlendFactor,
            BlendOperation, BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites,
            CompareFunction, DepthStencilState, Face, FilterMode, FragmentState, PipelineCache,
            RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
            StencilFaceState, StencilOperation, StencilState, TextureSampleType, VertexAttribute,
            VertexFormat, VertexState, VertexStepMode,
            binding_types::{sampler, texture_2d, texture_2d_array, uniform_buffer},
        },
        renderer::RenderDevice,
    },
};

use crate::{
    accumulate::MoonlySpotUniform,
    view_targets::{GBUFFER_DEPTH_FORMAT, GBUFFER_FORMAT},
};

#[derive(Resource)]
pub(crate) struct MoonlySpotPipeline {
    pub layout: BindGroupLayoutDescriptor,
    pub back_mask: CachedRenderPipelineId,
    pub front_mask: CachedRenderPipelineId,
    pub shade: CachedRenderPipelineId,
    pub volumetric: CachedRenderPipelineId,
    pub shadow_sampler: Sampler,
}

pub(crate) fn init_spot_pipeline(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    pipeline_cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
) {
    let layout = spot_bind_group_layout();
    let shader = load_embedded_asset!(asset_server.as_ref(), "accum_spot.wesl");
    let vertex = volume_vertex(shader.clone());
    let back_mask = pipeline_cache.queue_render_pipeline(mask_descriptor(
        &layout,
        vertex.clone(),
        Face::Front,
        0x01,
    ));
    let front_mask = pipeline_cache.queue_render_pipeline(mask_descriptor(
        &layout,
        vertex.clone(),
        Face::Back,
        0xff,
    ));
    let shade = pipeline_cache.queue_render_pipeline(shade_descriptor(
        &layout,
        vertex.clone(),
        shader.clone(),
    ));
    let volumetric =
        pipeline_cache.queue_render_pipeline(volumetric_descriptor(&layout, vertex, shader));
    let shadow_sampler = device.create_sampler(&SamplerDescriptor {
        compare: Some(CompareFunction::LessEqual),
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..Default::default()
    });
    commands.insert_resource(MoonlySpotPipeline {
        layout,
        back_mask,
        front_mask,
        shade,
        volumetric,
        shadow_sampler,
    });
}

fn spot_bind_group_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "moonly_spot_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX_FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<MoonlySpotUniform>(false),
                texture_2d(TextureSampleType::Depth),
                sampler(SamplerBindingType::Comparison),
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_2d_array(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
            ),
        ),
    )
}

fn volume_vertex(shader: Handle<Shader>) -> VertexState {
    VertexState {
        shader,
        buffers: vec![VertexBufferLayout {
            array_stride: size_of::<Vec3>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: vec![VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            }],
        }],
        ..default()
    }
}

fn mask_descriptor(
    layout: &BindGroupLayoutDescriptor,
    vertex: VertexState,
    cull: Face,
    read_mask: u32,
) -> RenderPipelineDescriptor {
    RenderPipelineDescriptor {
        label: Some("moonly_spot_stencil_mask".into()),
        layout: vec![layout.clone()],
        fragment: Some(FragmentState {
            shader: vertex.shader.clone(),
            entry_point: Some("mask_fragment".into()),
            targets: vec![Some(ColorTargetState {
                format: GBUFFER_FORMAT,
                blend: None,
                write_mask: ColorWrites::empty(),
            })],
            ..default()
        }),
        vertex,
        primitive: bevy::render::render_resource::PrimitiveState {
            cull_mode: Some(cull),
            ..default()
        },
        depth_stencil: Some(DepthStencilState {
            format: GBUFFER_DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(CompareFunction::GreaterEqual),
            stencil: StencilState {
                front: stencil_face(StencilOperation::Replace),
                back: stencil_face(StencilOperation::Replace),
                read_mask,
                write_mask: 0xff,
            },
            bias: default(),
        }),
        ..default()
    }
}

fn shade_descriptor(
    layout: &BindGroupLayoutDescriptor,
    vertex: VertexState,
    shader: Handle<Shader>,
) -> RenderPipelineDescriptor {
    let face = StencilFaceState {
        compare: CompareFunction::LessEqual,
        fail_op: StencilOperation::Keep,
        depth_fail_op: StencilOperation::Keep,
        pass_op: StencilOperation::Keep,
    };
    RenderPipelineDescriptor {
        label: Some("moonly_spot_accumulation".into()),
        layout: vec![layout.clone()],
        vertex,
        fragment: Some(FragmentState {
            shader,
            entry_point: Some("fragment".into()),
            targets: vec![Some(ColorTargetState {
                format: GBUFFER_FORMAT,
                blend: Some(BlendState {
                    color: additive_component(),
                    alpha: additive_component(),
                }),
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        primitive: bevy::render::render_resource::PrimitiveState {
            cull_mode: Some(Face::Front),
            ..default()
        },
        depth_stencil: Some(DepthStencilState {
            format: GBUFFER_DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(CompareFunction::Always),
            stencil: StencilState {
                front: face,
                back: face,
                read_mask: 0xff,
                write_mask: 0,
            },
            bias: default(),
        }),
        ..default()
    }
}

fn volumetric_descriptor(
    layout: &BindGroupLayoutDescriptor,
    vertex: VertexState,
    shader: Handle<Shader>,
) -> RenderPipelineDescriptor {
    RenderPipelineDescriptor {
        label: Some("moonly_spot_volumetric".into()),
        layout: vec![layout.clone()],
        vertex,
        fragment: Some(FragmentState {
            shader,
            entry_point: Some("volumetric_fragment".into()),
            targets: vec![Some(ColorTargetState {
                format: GBUFFER_FORMAT,
                blend: Some(BlendState {
                    color: additive_component(),
                    alpha: additive_component(),
                }),
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        primitive: bevy::render::render_resource::PrimitiveState {
            cull_mode: Some(Face::Front),
            ..default()
        },
        ..default()
    }
}

const fn stencil_face(depth_fail_op: StencilOperation) -> StencilFaceState {
    StencilFaceState {
        compare: CompareFunction::LessEqual,
        fail_op: StencilOperation::Keep,
        depth_fail_op,
        pass_op: StencilOperation::Keep,
    }
}

const fn additive_component() -> BlendComponent {
    BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    }
}

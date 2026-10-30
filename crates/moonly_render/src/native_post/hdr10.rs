use bevy::{
    asset::{AssetServer, Handle, load_embedded_asset},
    camera::CameraOutputMode,
    core_pipeline::FullscreenShader,
    ecs::system::SystemParam,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            BlendState, CachedRenderPipelineId, ColorTargetState, ColorWrites, FragmentState,
            PipelineCache, RenderPipelineDescriptor, Sampler, SamplerBindingType, ShaderStages,
            ShaderType, SpecializedRenderPipeline, SpecializedRenderPipelines, TextureFormat,
            TextureSampleType, UniformBuffer,
            binding_types::{sampler, texture_2d, uniform_buffer},
        },
        renderer::{RenderDevice, RenderQueue},
        view::ViewTarget,
    },
    shader::Shader,
};
use moonly_post::Hdr10PostConfig;

use crate::{
    MoonlyPostSettings,
    camera::{MoonlyCamera, MoonlyLinearOutput},
    view_targets::MoonlyViewTargets,
};

#[derive(Resource)]
pub(crate) struct MoonlyHdr10OutputPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    fullscreen: FullscreenShader,
    shader: Handle<Shader>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MoonlyHdr10OutputKey {
    format: TextureFormat,
    blend: Option<BlendState>,
}

#[derive(Component)]
pub(crate) struct ViewHdr10OutputPipeline(pub CachedRenderPipelineId);

#[derive(Component)]
pub(crate) struct ViewHdr10OutputBindGroup(pub BindGroup);

#[derive(ShaderType)]
struct MoonlyHdr10OutputUniform {
    display: Vec4,
}

pub(crate) fn init_hdr10_output_pipeline(
    mut commands: Commands,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_hdr10_output_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<MoonlyHdr10OutputUniform>(false),
            ),
        ),
    );
    commands.insert_resource(MoonlyHdr10OutputPipeline {
        layout,
        sampler: device.create_sampler(&default()),
        fullscreen: fullscreen.clone(),
        shader: load_embedded_asset!(asset_server.as_ref(), "hdr10_output.wesl"),
    });
}

impl SpecializedRenderPipeline for MoonlyHdr10OutputPipeline {
    type Key = MoonlyHdr10OutputKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("moonly_hdr10_output_pipeline".into()),
            layout: vec![self.layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                targets: vec![Some(ColorTargetState {
                    format: key.format,
                    blend: key.blend,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        }
    }
}

pub(crate) fn prepare_hdr10_output_pipelines(
    mut commands: Commands,
    cache: Res<PipelineCache>,
    pipeline: Res<MoonlyHdr10OutputPipeline>,
    mut pipelines: ResMut<SpecializedRenderPipelines<MoonlyHdr10OutputPipeline>>,
    views: Query<Hdr10PipelineView, With<MoonlyCamera>>,
) {
    for (entity, target, camera, linear) in &views {
        let format = target.out_texture_view_format();
        let hdr10 = format.is_some_and(|format| needs_hdr10_output(format, linear.is_some()));
        if !hdr10 {
            commands
                .entity(entity)
                .remove::<(ViewHdr10OutputPipeline, ViewHdr10OutputBindGroup)>();
            continue;
        }
        let Some(format) = format else {
            continue;
        };
        let key = MoonlyHdr10OutputKey {
            format,
            blend: output_blend_state(camera),
        };
        let id = pipelines.specialize(&cache, &pipeline, key);
        commands.entity(entity).insert(ViewHdr10OutputPipeline(id));
    }
}

type Hdr10PipelineView = (
    Entity,
    &'static ViewTarget,
    &'static ExtractedCamera,
    Option<&'static MoonlyLinearOutput>,
);

const fn needs_hdr10_output(format: TextureFormat, linear: bool) -> bool {
    matches!(format, TextureFormat::Rgb10a2Unorm) && !linear
}

fn output_blend_state(camera: &ExtractedCamera) -> Option<BlendState> {
    match camera.output_mode {
        CameraOutputMode::Skip => None,
        CameraOutputMode::Write { blend_state, .. } => blend_state.or_else(|| {
            (camera.sorted_camera_index_for_target > 0).then_some(BlendState::ALPHA_BLENDING)
        }),
    }
}

#[derive(SystemParam)]
pub(crate) struct Hdr10OutputPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    queue: Res<'w, RenderQueue>,
    cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyHdr10OutputPipeline>,
    config: Res<'w, Hdr10PostConfig>,
}

pub(crate) fn prepare_hdr10_output_bind_groups(
    mut commands: Commands,
    inputs: Hdr10OutputPrepareInputs,
    views: Query<Hdr10BindGroupView, With<MoonlyCamera>>,
) {
    for (entity, target, targets, linear, settings) in &views {
        let Some(format) = target.out_texture_view_format() else {
            continue;
        };
        if !needs_hdr10_output(format, linear.is_some()) {
            continue;
        }
        let max_nits = settings.map_or(inputs.config.max_nits, |settings| settings.max_nits);
        let mut uniform = UniformBuffer::from(MoonlyHdr10OutputUniform {
            display: Vec4::new(max_nits, 0.0, 0.0, 0.0),
        });
        uniform.write_buffer(&inputs.device, &inputs.queue);
        let group = inputs.device.create_bind_group(
            Some("moonly_hdr10_output_bind_group"),
            &inputs.cache.get_bind_group_layout(&inputs.pipeline.layout),
            &BindGroupEntries::sequential((
                &targets.post_process.texture.default_view,
                &inputs.pipeline.sampler,
                &uniform,
            )),
        );
        commands
            .entity(entity)
            .insert(ViewHdr10OutputBindGroup(group));
    }
}

type Hdr10BindGroupView = (
    Entity,
    &'static ViewTarget,
    &'static MoonlyViewTargets,
    Option<&'static MoonlyLinearOutput>,
    Option<&'static MoonlyPostSettings>,
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hdr10_output_only_replaces_the_rgb10a2_final_blit() {
        assert!(needs_hdr10_output(TextureFormat::Rgb10a2Unorm, false));
        assert!(!needs_hdr10_output(TextureFormat::Rgba16Float, false));
        assert!(!needs_hdr10_output(TextureFormat::Bgra8UnormSrgb, false));
        assert!(!needs_hdr10_output(TextureFormat::Rgb10a2Unorm, true));
    }
}

use std::sync::{Arc, atomic::Ordering};

use bevy::{
    asset::{AssetServer, Handle, load_embedded_asset},
    ecs::system::SystemParamItem,
    material::{MaterialProperties, key::ErasedMaterialPipelineKey},
    pbr::{PrepassPipeline, PrepassPipelineSpecializer},
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            ColorTargetState, ColorWrites, FragmentState, PipelineCache, RenderPipelineDescriptor,
            ShaderStages, ShaderType, SpecializedMeshPipeline, SpecializedMeshPipelineError,
            TextureFormat, UniformBuffer, binding_types::uniform_buffer,
        },
        renderer::RenderDevice,
    },
    shader::Shader,
};
use moonly_core::SpotShadowXform;

use crate::{
    shadow_targets::{MoonlyShadowViewIndex, MoonlySpotShadowTarget},
    surface::MoonlySurfaceLayout,
    view_targets::DEPTH_FORMAT,
};

#[derive(ShaderType)]
pub(crate) struct MoonlyShadowViewUniform {
    clip_from_world: Mat4,
}

impl From<&SpotShadowXform> for MoonlyShadowViewUniform {
    fn from(value: &SpotShadowXform) -> Self {
        Self {
            clip_from_world: value.combine,
        }
    }
}

#[derive(Resource, Clone)]
pub(crate) struct MoonlyShadowPipeline {
    prepass: PrepassPipeline,
    view_layout: BindGroupLayoutDescriptor,
    surface_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
}

pub(crate) fn init_shadow_pipeline(
    mut commands: Commands,
    prepass: Res<PrepassPipeline>,
    surface: Res<MoonlySurfaceLayout>,
    asset_server: Res<AssetServer>,
) {
    let view_layout = BindGroupLayoutDescriptor::new(
        "moonly_shadow_view_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::VERTEX,
            (uniform_buffer::<MoonlyShadowViewUniform>(false),),
        ),
    );
    commands.insert_resource(MoonlyShadowPipeline {
        prepass: prepass.clone(),
        view_layout,
        surface_layout: surface.0.clone(),
        shader: load_embedded_asset!(asset_server.as_ref(), "shadow_depth.wesl"),
    });
}

impl MoonlyShadowPipeline {
    pub(crate) fn create_view_bind_group(
        &self,
        device: &RenderDevice,
        cache: &PipelineCache,
        uniform: &UniformBuffer<MoonlyShadowViewUniform>,
    ) -> BindGroup {
        device.create_bind_group(
            Some("moonly_shadow_view_bind_group"),
            &cache.get_bind_group_layout(&self.view_layout),
            &BindGroupEntries::sequential((uniform,)),
        )
    }
}

pub(crate) struct MoonlyShadowMeshPipeline {
    prepass: PrepassPipelineSpecializer,
    pipeline: MoonlyShadowPipeline,
}

impl MoonlyShadowMeshPipeline {
    pub(crate) fn new(
        pipeline: &MoonlyShadowPipeline,
        properties: &Arc<MaterialProperties>,
    ) -> Self {
        Self {
            prepass: PrepassPipelineSpecializer {
                pipeline: pipeline.prepass.clone(),
                properties: properties.clone(),
            },
            pipeline: pipeline.clone(),
        }
    }
}

impl SpecializedMeshPipeline for MoonlyShadowMeshPipeline {
    type Key = ErasedMaterialPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.prepass.specialize(key, layout)?;
        descriptor.label = Some("moonly_shadow_depth_pipeline".into());
        descriptor.vertex.shader = self.pipeline.shader.clone();
        let shader_defs = descriptor.vertex.shader_defs.clone();
        descriptor.fragment = Some(FragmentState {
            shader: self.pipeline.shader.clone(),
            shader_defs,
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        });
        descriptor.layout.push(self.pipeline.view_layout.clone());
        descriptor.layout.push(self.pipeline.surface_layout.clone());
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.format = DEPTH_FORMAT;
            depth.depth_compare = Some(bevy::render::render_resource::CompareFunction::LessEqual);
        }
        Ok(descriptor)
    }
}

pub(crate) struct SetMoonlyShadowViewBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMoonlyShadowViewBindGroup<I> {
    type Param = ();
    type ViewQuery = (
        &'static MoonlySpotShadowTarget,
        &'static MoonlyShadowViewIndex,
    );
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        (target, index): (&'w MoonlySpotShadowTarget, &'w MoonlyShadowViewIndex),
        _entity: Option<()>,
        _param: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let batches = if index.domain.load(Ordering::Relaxed) == 0 {
            &target.batches
        } else {
            &target.sun_batches
        };
        let bind_group = batches
            .get(index.batch.load(Ordering::Relaxed))
            .and_then(|batch| batch.views.get(index.view.load(Ordering::Relaxed)))
            .map(|view| &view.bind_group);
        let Some(bind_group) = bind_group else {
            return RenderCommandResult::Failure("Moonly shadow view is not prepared");
        };
        pass.set_bind_group(I, bind_group, &[]);
        RenderCommandResult::Success
    }
}

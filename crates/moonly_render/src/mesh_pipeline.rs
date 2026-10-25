use std::sync::Arc;

use bevy::{
    asset::Handle,
    material::key::ErasedMaterialPipelineKey,
    pbr::PrepassPipelineSpecializer,
    prelude::*,
    render::{
        mesh::MeshVertexBufferLayoutRef,
        render_resource::{
            ColorTargetState, ColorWrites, CompareFunction, RenderPipelineDescriptor,
            SpecializedMeshPipeline, SpecializedMeshPipelineError, StencilFaceState,
            StencilOperation, StencilState,
        },
    },
    shader::Shader,
};

use crate::view_targets::{GBUFFER_DEPTH_FORMAT, GBUFFER_FORMAT};

pub(crate) struct MoonlyMeshPipeline {
    pub prepass: PrepassPipelineSpecializer,
    pub shader: Handle<Shader>,
    pub surface_layout: bevy::render::render_resource::BindGroupLayoutDescriptor,
}

impl MoonlyMeshPipeline {
    pub(crate) fn new(
        prepass: bevy::pbr::PrepassPipeline,
        properties: &Arc<bevy::material::MaterialProperties>,
        shader: Handle<Shader>,
        surface_layout: &bevy::render::render_resource::BindGroupLayoutDescriptor,
    ) -> Self {
        Self {
            prepass: PrepassPipelineSpecializer {
                pipeline: prepass,
                properties: properties.clone(),
            },
            shader,
            surface_layout: surface_layout.clone(),
        }
    }
}

impl SpecializedMeshPipeline for MoonlyMeshPipeline {
    type Key = ErasedMaterialPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.prepass.specialize(key, layout)?;
        descriptor.label = Some("moonly_gbuffer_pipeline".into());
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.format = GBUFFER_DEPTH_FORMAT;
            let face = StencilFaceState {
                compare: CompareFunction::Always,
                fail_op: StencilOperation::Keep,
                depth_fail_op: StencilOperation::Keep,
                pass_op: StencilOperation::Replace,
            };
            depth.stencil = StencilState {
                front: face,
                back: face,
                read_mask: 0xff,
                write_mask: 0xff,
            };
        }
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = self.shader.clone();
            fragment.shader_defs.push("MOONLY_GBUFFER".into());
            fragment.targets = gbuffer_targets();
        }
        descriptor.layout.push(self.surface_layout.clone());
        Ok(descriptor)
    }
}

fn gbuffer_targets() -> Vec<Option<ColorTargetState>> {
    (0..4)
        .map(|_| {
            Some(ColorTargetState {
                format: GBUFFER_FORMAT,
                blend: None,
                write_mask: ColorWrites::ALL,
            })
        })
        .collect()
}

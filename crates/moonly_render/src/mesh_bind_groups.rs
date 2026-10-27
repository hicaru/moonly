use std::collections::HashMap;

use bevy::{
    ecs::{
        query::ROQueryItem,
        system::{SystemParam, SystemParamItem, lifetimeless::SRes},
    },
    pbr::{
        MeshBindGroups, MeshPipeline, MeshUniform, MorphIndices, RenderLightmaps,
        RenderMeshInstances, SetMeshBindGroup, SkinUniforms,
    },
    prelude::*,
    render::{
        mesh::{
            MeshMetadataFallbackBuffer, RenderMesh, RenderMeshBufferInfo,
            allocator::{MeshAllocator, MeshSlabId},
        },
        render_asset::RenderAssets,
        render_phase::{
            PhaseItem, PhaseItemExtraIndex, RenderCommand, RenderCommandResult, TrackedRenderPass,
        },
        render_resource::{BindGroup, PipelineCache},
        renderer::RenderDevice,
    },
};

use crate::phase::MoonlyOpaque3d;

#[derive(Resource, Default)]
pub(crate) struct MoonlyMeshBindGroups {
    model_only: HashMap<MeshSlabId, BindGroup>,
    skinned: HashMap<MeshSlabId, BindGroup>,
}

#[derive(SystemParam)]
pub(crate) struct MoonlyBindGroupInputs<'w> {
    phase_buffers: Res<
        'w,
        bevy::render::batching::gpu_preprocessing::PhaseBatchedInstanceBuffers<
            MoonlyOpaque3d,
            MeshUniform,
        >,
    >,
    pipeline: Res<'w, MeshPipeline>,
    device: Res<'w, RenderDevice>,
    pipeline_cache: Res<'w, PipelineCache>,
    allocator: Res<'w, MeshAllocator>,
    fallback: Res<'w, MeshMetadataFallbackBuffer>,
    skins: Res<'w, SkinUniforms>,
}

pub(crate) fn prepare_moonly_mesh_bind_groups(
    inputs: MoonlyBindGroupInputs,
    mut groups: ResMut<MoonlyMeshBindGroups>,
) {
    groups.model_only.clear();
    groups.skinned.clear();
    let Some(model) = inputs.phase_buffers.buffers.instance_data_binding() else {
        return;
    };
    for slab in inputs.allocator.metadata_slabs() {
        let metadata = inputs
            .allocator
            .buffer_for_slab(slab)
            .unwrap_or(&inputs.fallback.buffer);
        let model_group = inputs.pipeline.mesh_layouts.model_only(
            &inputs.device,
            &inputs.pipeline_cache,
            &model,
            metadata,
        );
        let skinned_group = inputs.pipeline.mesh_layouts.skinned(
            &inputs.device,
            &inputs.pipeline_cache,
            &model,
            metadata,
            &inputs.skins.current_buffer,
        );
        groups.model_only.insert(slab, model_group);
        groups.skinned.insert(slab, skinned_group);
    }
}

pub(crate) struct SetMoonlyMeshBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMoonlyMeshBindGroup<I> {
    type Param = (
        SRes<MoonlyMeshBindGroups>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
        SRes<MeshMetadataFallbackBuffer>,
        SRes<SkinUniforms>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        (groups, instances, allocator, fallback, skins): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let groups = groups.into_inner();
        let Some(mesh_id) = instances.mesh_asset_id(item.main_entity()) else {
            return RenderCommandResult::Skip;
        };
        let slab = allocator
            .mesh_slabs(&mesh_id)
            .and_then(|slabs| slabs.metadata_slab_id)
            .unwrap_or(fallback.slab_id);
        let bind_group = if skins.skin_byte_offset(item.main_entity()).is_some() {
            groups.skinned.get(&slab)
        } else {
            groups.model_only.get(&slab)
        };
        let Some(bind_group) = bind_group else {
            return RenderCommandResult::Skip;
        };
        if let PhaseItemExtraIndex::DynamicOffset(offset) = item.extra_index() {
            pass.set_bind_group(I, bind_group, &[offset]);
        } else {
            pass.set_bind_group(I, bind_group, &[]);
        }
        RenderCommandResult::Success
    }
}

pub(crate) struct SetMoonlyMotionMeshBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMoonlyMotionMeshBindGroup<I> {
    type Param = (
        SRes<RenderDevice>,
        SRes<MeshBindGroups>,
        SRes<RenderMeshInstances>,
        SRes<SkinUniforms>,
        SRes<MorphIndices>,
        SRes<MeshAllocator>,
        SRes<RenderLightmaps>,
        SRes<MeshMetadataFallbackBuffer>,
    );
    type ViewQuery = Has<bevy::core_pipeline::prepass::MotionVectorPrepass>;
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        motion_vectors: bool,
        entity: Option<()>,
        params: SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        if !motion_vectors {
            return RenderCommandResult::Success;
        }
        SetMeshBindGroup::<I>::render(item, motion_vectors, entity, params, pass)
    }
}

pub(crate) struct DrawMoonlyMeshDirect;

impl<P: PhaseItem> RenderCommand<P> for DrawMoonlyMeshDirect {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        (meshes, instances, allocator): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let meshes = meshes.into_inner();
        let allocator = allocator.into_inner();
        let Some(mesh_id) = instances.mesh_asset_id(item.main_entity()) else {
            return RenderCommandResult::Skip;
        };
        let Some(gpu_mesh) = meshes.get(mesh_id) else {
            return RenderCommandResult::Skip;
        };
        let Some(vertices) = allocator.mesh_vertex_slice(&mesh_id) else {
            return RenderCommandResult::Skip;
        };
        pass.set_vertex_buffer(0, vertices.buffer.slice(..));
        match &gpu_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                index_format,
                count,
            } => {
                let Some(indices) = allocator.mesh_index_slice(&mesh_id) else {
                    return RenderCommandResult::Skip;
                };
                pass.set_index_buffer(indices.buffer.slice(..), *index_format);
                let range = indices.range.start..(indices.range.start + *count);
                pass.draw_indexed(
                    range,
                    vertices.range.start as i32,
                    item.batch_range().clone(),
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(vertices.range, item.batch_range().clone());
            }
        }
        RenderCommandResult::Success
    }
}

pub(crate) type DrawMoonlyMesh = (
    bevy::render::render_phase::SetItemPipeline,
    bevy::pbr::SetPrepassViewBindGroup<0>,
    bevy::pbr::SetPrepassViewEmptyBindGroup<1>,
    SetMoonlyMeshBindGroup<2>,
    SetMoonlyMotionMeshBindGroup<2>,
    bevy::pbr::SetMaterialBindGroup<3>,
    crate::surface::SetMoonlySurfaceBindGroup<4>,
    DrawMoonlyMeshDirect,
);

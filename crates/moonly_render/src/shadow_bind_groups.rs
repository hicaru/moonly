use std::collections::HashMap;

use bevy::{
    ecs::{
        query::ROQueryItem,
        system::{SystemParam, SystemParamItem, lifetimeless::SRes},
    },
    pbr::{MeshPipeline, MeshUniform, RenderMeshInstances, SkinUniforms},
    prelude::*,
    render::{
        batching::gpu_preprocessing::PhaseBatchedInstanceBuffers,
        mesh::{
            MeshMetadataFallbackBuffer,
            allocator::{MeshAllocator, MeshSlabId},
        },
        render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
        render_resource::{BindGroup, PipelineCache},
        renderer::RenderDevice,
    },
};

use crate::{
    caster_visibility::MoonlyCasterVisibility,
    mesh_bounds::{MoonlyMeshBounds, sphere_visible},
    phase::MoonlyShadow3d,
    shadow_targets::{MoonlyShadowViewIndex, MoonlySpotShadowTarget},
};

#[derive(Resource, Default)]
pub(crate) struct MoonlyShadowMeshBindGroups {
    model_only: HashMap<MeshSlabId, BindGroup>,
    skinned: HashMap<MeshSlabId, BindGroup>,
}

#[derive(SystemParam)]
pub(crate) struct ShadowBindGroupInputs<'w> {
    phase_buffers: Res<'w, PhaseBatchedInstanceBuffers<MoonlyShadow3d, MeshUniform>>,
    pipeline: Res<'w, MeshPipeline>,
    device: Res<'w, RenderDevice>,
    pipeline_cache: Res<'w, PipelineCache>,
    allocator: Res<'w, MeshAllocator>,
    fallback: Res<'w, MeshMetadataFallbackBuffer>,
    skins: Res<'w, SkinUniforms>,
}

pub(crate) fn prepare_shadow_mesh_bind_groups(
    inputs: ShadowBindGroupInputs,
    mut groups: ResMut<MoonlyShadowMeshBindGroups>,
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

pub(crate) struct CullMoonlyShadow;

impl<P: PhaseItem> RenderCommand<P> for CullMoonlyShadow {
    type Param = (
        SRes<RenderMeshInstances>,
        SRes<SkinUniforms>,
        SRes<MoonlyMeshBounds>,
        SRes<MoonlyCasterVisibility>,
    );
    type ViewQuery = (
        &'static MoonlySpotShadowTarget,
        &'static MoonlyShadowViewIndex,
    );
    type ItemQuery = &'static GlobalTransform;

    fn render<'w>(
        item: &P,
        (target, index): ROQueryItem<'w, '_, Self::ViewQuery>,
        transform: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (instances, skins, bounds, caster_visibility): SystemParamItem<'w, '_, Self::Param>,
        _pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(mesh_id) = instances.mesh_asset_id(item.main_entity()) else {
            return RenderCommandResult::Skip;
        };
        let active_view = target.active_view(index);
        let occluded = active_view.is_some_and(|view| {
            view.source_entity.is_some_and(|source| {
                caster_visibility.is_caster_invisible(source, item.main_entity().id())
            })
        });
        if occluded {
            return RenderCommandResult::Skip;
        }
        let skinned = skins.skin_byte_offset(item.main_entity()).is_some();
        if !skinned {
            let visible = match (bounds.get(mesh_id), transform, active_view) {
                (Some(aabb), Some(transform), Some(view)) => {
                    sphere_visible(aabb, transform, view.xform.combine)
                }
                _ => true,
            };
            if !visible {
                return RenderCommandResult::Skip;
            }
        }
        RenderCommandResult::Success
    }
}

pub(crate) struct SetMoonlyShadowMeshBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMoonlyShadowMeshBindGroup<I> {
    type Param = (
        SRes<MoonlyShadowMeshBindGroups>,
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
        pass.set_bind_group(I, bind_group, &[]);
        RenderCommandResult::Success
    }
}

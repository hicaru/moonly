use std::{any::TypeId, collections::HashMap};

use bevy::{
    core_pipeline::prepass::{
        MotionVectorPrepass, OpaqueNoLightmap3dBatchSetKey, OpaqueNoLightmap3dBinKey,
    },
    ecs::system::SystemParam,
    material::{
        RenderPhaseType,
        key::{ErasedMaterialPipelineKey, ErasedMeshPipelineKey},
    },
    mesh::Mesh3d,
    pbr::{
        MeshPipelineKey, PreparedMaterial, RenderMaterialInstances, RenderMeshInstanceFlags,
        RenderMeshInstances, SetMaterialBindGroup, SetPrepassViewBindGroup, StandardMaterial,
        alpha_mode_pipeline_key,
    },
    prelude::*,
    render::{
        erased_render_asset::ErasedRenderAssets,
        mesh::allocator::MeshAllocator,
        render_asset::RenderAssets,
        render_phase::{
            BinnedRenderPhase, BinnedRenderPhaseType, DrawFunctions, SetItemPipeline,
            ViewBinnedRenderPhases,
        },
        render_resource::{PipelineCache, SpecializedMeshPipelines},
        sync_world::{MainEntity, MainEntityHashSet},
        view::{ExtractedView, RenderVisibleEntities, RetainedViewEntity},
    },
};

use crate::{
    camera::MoonlyCamera,
    mesh_bind_groups::{DrawMoonlyMeshDirect, SetMoonlyMotionMeshBindGroup},
    phase::MoonlyShadow3d,
    shadow_bind_groups::{CullMoonlyShadow, SetMoonlyShadowMeshBindGroup},
    shadow_pipeline::{
        MoonlyShadowMeshPipeline, MoonlyShadowPipeline, SetMoonlyShadowViewBindGroup,
    },
    surface::SetMoonlySurfaceBindGroup,
};

pub(crate) type DrawMoonlyShadow = (
    SetItemPipeline,
    SetPrepassViewBindGroup<0>,
    SetPrepassViewBindGroup<1>,
    SetMoonlyShadowViewBindGroup<4>,
    SetMoonlySurfaceBindGroup<5>,
    CullMoonlyShadow,
    SetMoonlyShadowMeshBindGroup<2>,
    SetMoonlyMotionMeshBindGroup<2>,
    SetMaterialBindGroup<3>,
    DrawMoonlyMeshDirect,
);

#[derive(Resource, Default)]
pub(crate) struct MoonlyShadowQueuedEntities(HashMap<RetainedViewEntity, MainEntityHashSet>);

#[derive(SystemParam)]
pub(crate) struct ShadowQueueResources<'w> {
    meshes: Res<'w, RenderAssets<bevy::render::mesh::RenderMesh>>,
    materials: Res<'w, ErasedRenderAssets<PreparedMaterial>>,
    instances: Res<'w, RenderMeshInstances>,
    material_instances: Res<'w, RenderMaterialInstances>,
    allocator: Res<'w, MeshAllocator>,
    pipeline_cache: Res<'w, PipelineCache>,
    pipeline: Res<'w, MoonlyShadowPipeline>,
    adapters: Res<'w, crate::MoonlyMaterialAdapters>,
}

type ViewQuery = (
    &'static ExtractedView,
    &'static RenderVisibleEntities,
    Has<MotionVectorPrepass>,
);

#[derive(SystemParam)]
pub(crate) struct ShadowViewQueues<'w, 's> {
    pipelines: ResMut<'w, SpecializedMeshPipelines<MoonlyShadowMeshPipeline>>,
    phases: ResMut<'w, ViewBinnedRenderPhases<MoonlyShadow3d>>,
    draws: Res<'w, DrawFunctions<MoonlyShadow3d>>,
    queued: ResMut<'w, MoonlyShadowQueuedEntities>,
    views: Query<'w, 's, ViewQuery, With<MoonlyCamera>>,
}

struct ShadowQueueContext<'a, 'w> {
    resources: &'a ShadowQueueResources<'w>,
    pipelines: &'a mut SpecializedMeshPipelines<MoonlyShadowMeshPipeline>,
    phase: &'a mut BinnedRenderPhase<MoonlyShadow3d>,
    queued: &'a mut MainEntityHashSet,
    draw: bevy::render::render_phase::DrawFunctionId,
    motion_vectors: bool,
}

pub(crate) fn queue_shadow_meshes(resources: ShadowQueueResources, mut queues: ShadowViewQueues) {
    let draw = queues.draws.read().id::<DrawMoonlyShadow>();
    for (view, visible, motion_vectors) in &queues.views {
        let key = view.retained_view_entity;
        let Some(phase) = queues.phases.get_mut(&key) else {
            continue;
        };
        let queued = queues.queued.0.entry(key).or_default();
        for entity in queued.drain() {
            phase.remove(entity);
        }
        let mut context = ShadowQueueContext {
            resources: &resources,
            pipelines: &mut queues.pipelines,
            phase,
            queued,
            draw,
            motion_vectors,
        };
        let Some(entities) = visible.get::<Mesh3d>() else {
            continue;
        };
        for (render_entity, main_entity) in entities.iter_visible() {
            queue_shadow_entity(*render_entity, *main_entity, &mut context);
        }
    }
    queues
        .queued
        .0
        .retain(|view, _| queues.phases.contains_key(view));
}

fn queue_shadow_entity(
    render_entity: Entity,
    main_entity: MainEntity,
    context: &mut ShadowQueueContext,
) {
    let resources = context.resources;
    let Some(material_instance) = resources.material_instances.instances.get(&main_entity) else {
        return;
    };
    let material_type = material_instance.asset_id.type_id();
    if material_type != TypeId::of::<StandardMaterial>()
        && resources.adapters.get(material_type).is_none()
    {
        return;
    }
    let Some(mesh_instance) = resources.instances.render_mesh_queue_data(main_entity) else {
        return;
    };
    if mesh_instance.lightmap_slab_index().is_some()
        || !mesh_instance
            .flags()
            .contains(RenderMeshInstanceFlags::SHADOW_CASTER)
    {
        return;
    }
    let Some(material) = resources.materials.get(material_instance.asset_id) else {
        return;
    };
    if !matches!(
        material.properties.render_phase_type,
        RenderPhaseType::Opaque | RenderPhaseType::AlphaMask
    ) {
        return;
    }
    let Some(mesh) = resources.meshes.get(mesh_instance.mesh_asset_id()) else {
        return;
    };
    let Some(slabs) = resources
        .allocator
        .mesh_slabs(&mesh_instance.mesh_asset_id())
    else {
        return;
    };
    let mesh_key = MeshPipelineKey::from_msaa_samples(1)
        | MeshPipelineKey::DEPTH_PREPASS
        | MeshPipelineKey::from_bits_retain(mesh.key_bits.bits())
        | material.properties.mesh_pipeline_key_bits.downcast()
        | alpha_mode_pipeline_key(material.properties.alpha_mode, &Msaa::Off)
        | crate::motion_vectors::pipeline_key(context.motion_vectors, mesh_instance.flags());
    let key = ErasedMaterialPipelineKey {
        type_id: material_instance.asset_id.type_id(),
        mesh_key: ErasedMeshPipelineKey::new(mesh_key),
        material_key: material.properties.material_key.clone(),
    };
    let specializer = MoonlyShadowMeshPipeline::new(&resources.pipeline, &material.properties);
    let Ok(pipeline) =
        context
            .pipelines
            .specialize(&resources.pipeline_cache, &specializer, key, &mesh.layout)
    else {
        return;
    };
    context.phase.add(
        OpaqueNoLightmap3dBatchSetKey {
            draw_function: context.draw,
            pipeline,
            material_bind_group_index: Some(material.binding.group.0),
            slabs,
        },
        OpaqueNoLightmap3dBinKey {
            asset_id: mesh_instance.mesh_asset_id().into(),
        },
        (render_entity, main_entity),
        mesh_instance.current_uniform_index,
        BinnedRenderPhaseType::UnbatchableMesh,
    );
    context.queued.insert(main_entity);
}

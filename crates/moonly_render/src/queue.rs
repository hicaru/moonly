use std::{any::TypeId, collections::HashMap};

use bevy::{
    asset::{AssetId, Handle},
    core_pipeline::prepass::{OpaqueNoLightmap3dBatchSetKey, OpaqueNoLightmap3dBinKey},
    ecs::system::SystemParam,
    material::{
        RenderPhaseType,
        key::{ErasedMaterialPipelineKey, ErasedMeshPipelineKey},
    },
    mesh::{Mesh, Mesh3d},
    pbr::{
        MeshPipelineKey, PreparedMaterial, PrepassPipeline, RenderMaterialInstances,
        RenderMeshInstances, StandardMaterial, alpha_mode_pipeline_key,
    },
    prelude::*,
    render::{
        erased_render_asset::ErasedRenderAssets,
        mesh::{
            RenderMesh,
            allocator::{MeshAllocator, MeshSlabs},
        },
        render_asset::RenderAssets,
        render_phase::{
            BinnedRenderPhase, BinnedRenderPhaseType, DrawFunctionId, DrawFunctions,
            InputUniformIndex, ViewBinnedRenderPhases,
        },
        render_resource::{CachedRenderPipelineId, PipelineCache, SpecializedMeshPipelines},
        sync_world::{MainEntity, MainEntityHashSet},
        view::{ExtractedView, RenderVisibilityRanges, RenderVisibleEntities, RetainedViewEntity},
    },
    shader::Shader,
};

use crate::{
    camera::MoonlyCamera,
    mesh_bind_groups::DrawMoonlyMesh,
    mesh_pipeline::MoonlyMeshPipeline,
    phase::{MoonlyAlphaMask3d, MoonlyOpaque3d},
};

#[derive(Resource, Clone)]
pub(crate) struct MoonlyGBufferShader(pub Handle<Shader>);

#[derive(Resource, Default)]
pub(crate) struct MoonlyQueuedEntities(HashMap<RetainedViewEntity, MainEntityHashSet>);

#[derive(SystemParam)]
pub(crate) struct MoonlyQueueResources<'w> {
    meshes: Res<'w, RenderAssets<RenderMesh>>,
    materials: Res<'w, ErasedRenderAssets<PreparedMaterial>>,
    instances: Res<'w, RenderMeshInstances>,
    material_instances: Res<'w, RenderMaterialInstances>,
    allocator: Res<'w, MeshAllocator>,
    visibility_ranges: Res<'w, RenderVisibilityRanges>,
    prepass: Res<'w, PrepassPipeline>,
    pipeline_cache: Res<'w, PipelineCache>,
    shader: Res<'w, MoonlyGBufferShader>,
    surface_layout: Res<'w, crate::surface::MoonlySurfaceLayout>,
}

#[derive(SystemParam)]
pub(crate) struct MoonlyViewQueues<'w, 's> {
    pipelines: ResMut<'w, SpecializedMeshPipelines<MoonlyMeshPipeline>>,
    opaque_phases: ResMut<'w, ViewBinnedRenderPhases<MoonlyOpaque3d>>,
    alpha_phases: ResMut<'w, ViewBinnedRenderPhases<MoonlyAlphaMask3d>>,
    opaque_draws: Res<'w, DrawFunctions<MoonlyOpaque3d>>,
    alpha_draws: Res<'w, DrawFunctions<MoonlyAlphaMask3d>>,
    queued: ResMut<'w, MoonlyQueuedEntities>,
    views:
        Query<'w, 's, (&'static ExtractedView, &'static RenderVisibleEntities), With<MoonlyCamera>>,
}

struct QueueViewContext<'a, 'w> {
    resources: &'a MoonlyQueueResources<'w>,
    pipelines: &'a mut SpecializedMeshPipelines<MoonlyMeshPipeline>,
    opaque_phase: &'a mut BinnedRenderPhase<MoonlyOpaque3d>,
    alpha_phase: &'a mut BinnedRenderPhase<MoonlyAlphaMask3d>,
    queued: &'a mut MainEntityHashSet,
    opaque_draw: DrawFunctionId,
    alpha_draw: DrawFunctionId,
}

pub(crate) fn queue_moonly_meshes(resources: MoonlyQueueResources, mut queues: MoonlyViewQueues) {
    let opaque_draw = queues.opaque_draws.read().id::<DrawMoonlyMesh>();
    let alpha_draw = queues.alpha_draws.read().id::<DrawMoonlyMesh>();
    for (view, visible) in &queues.views {
        let key = view.retained_view_entity;
        let (Some(opaque_phase), Some(alpha_phase)) = (
            queues.opaque_phases.get_mut(&key),
            queues.alpha_phases.get_mut(&key),
        ) else {
            continue;
        };
        let queued = queues.queued.0.entry(key).or_default();
        remove_previous(opaque_phase, alpha_phase, queued);
        let mut context = QueueViewContext {
            resources: &resources,
            pipelines: &mut queues.pipelines,
            opaque_phase,
            alpha_phase,
            queued,
            opaque_draw,
            alpha_draw,
        };
        let Some(entities) = visible.get::<Mesh3d>() else {
            continue;
        };
        for (render_entity, main_entity) in entities.iter_visible() {
            queue_entity(*render_entity, *main_entity, &mut context);
        }
    }
    queues
        .queued
        .0
        .retain(|view, _| queues.opaque_phases.contains_key(view));
}

fn remove_previous(
    opaque: &mut BinnedRenderPhase<MoonlyOpaque3d>,
    alpha: &mut BinnedRenderPhase<MoonlyAlphaMask3d>,
    queued: &mut MainEntityHashSet,
) {
    for entity in queued.drain() {
        opaque.remove(entity);
        alpha.remove(entity);
    }
}

fn queue_entity(render_entity: Entity, main_entity: MainEntity, context: &mut QueueViewContext) {
    let resources = context.resources;
    let Some(material_instance) = resources.material_instances.instances.get(&main_entity) else {
        return;
    };
    if material_instance.asset_id.type_id() != TypeId::of::<StandardMaterial>() {
        return;
    }
    let Some(mesh_instance) = resources.instances.render_mesh_queue_data(main_entity) else {
        return;
    };
    if mesh_instance.lightmap_slab_index().is_some() {
        return;
    }
    let Some(material) = resources.materials.get(material_instance.asset_id) else {
        return;
    };
    if !material_supports_gbuffer(&material.properties) {
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
    let mesh_key = create_mesh_key(main_entity, mesh, &material.properties, resources);
    let key = ErasedMaterialPipelineKey {
        type_id: material_instance.asset_id.type_id(),
        mesh_key: ErasedMeshPipelineKey::new(mesh_key),
        material_key: material.properties.material_key.clone(),
    };
    let specializer = MoonlyMeshPipeline::new(
        resources.prepass.clone(),
        &material.properties,
        resources.shader.0.clone(),
        &resources.surface_layout.0,
    );
    let Ok(pipeline) =
        context
            .pipelines
            .specialize(&resources.pipeline_cache, &specializer, key, &mesh.layout)
    else {
        return;
    };
    let item = QueuedMesh {
        pipeline,
        material_group: material.binding.group.0,
        slabs,
        mesh_id: mesh_instance.mesh_asset_id(),
        uniform_index: mesh_instance.current_uniform_index,
        phase_type: BinnedRenderPhaseType::UnbatchableMesh,
    };
    item.add(
        render_entity,
        main_entity,
        material.properties.render_phase_type,
        context,
    );
    context.queued.insert(main_entity);
}

const fn material_supports_gbuffer(properties: &bevy::material::MaterialProperties) -> bool {
    properties.prepass_enabled
        && !properties.reads_view_transmission_texture
        && matches!(
            properties.render_phase_type,
            RenderPhaseType::Opaque | RenderPhaseType::AlphaMask
        )
}

fn create_mesh_key(
    entity: MainEntity,
    mesh: &RenderMesh,
    properties: &bevy::material::MaterialProperties,
    resources: &MoonlyQueueResources,
) -> MeshPipelineKey {
    let mut key = MeshPipelineKey::from_msaa_samples(1)
        | MeshPipelineKey::DEFERRED_PREPASS
        | MeshPipelineKey::from_bits_retain(mesh.key_bits.bits())
        | properties.mesh_pipeline_key_bits.downcast()
        | alpha_mode_pipeline_key(properties.alpha_mode, &Msaa::Off);
    if resources
        .visibility_ranges
        .entity_has_crossfading_visibility_ranges(entity)
    {
        key |= MeshPipelineKey::VISIBILITY_RANGE_DITHER;
    }
    key
}

struct QueuedMesh {
    pipeline: CachedRenderPipelineId,
    material_group: u32,
    slabs: MeshSlabs,
    mesh_id: AssetId<Mesh>,
    uniform_index: InputUniformIndex,
    phase_type: BinnedRenderPhaseType,
}

impl QueuedMesh {
    fn add(
        &self,
        render_entity: Entity,
        main_entity: MainEntity,
        phase_type: RenderPhaseType,
        context: &mut QueueViewContext,
    ) {
        let draw = match phase_type {
            RenderPhaseType::Opaque => context.opaque_draw,
            _ => context.alpha_draw,
        };
        let batch = OpaqueNoLightmap3dBatchSetKey {
            draw_function: draw,
            pipeline: self.pipeline,
            material_bind_group_index: Some(self.material_group),
            slabs: self.slabs,
        };
        let bin = OpaqueNoLightmap3dBinKey {
            asset_id: self.mesh_id.into(),
        };
        let entities = (render_entity, main_entity);
        if matches!(phase_type, RenderPhaseType::Opaque) {
            context
                .opaque_phase
                .add(batch, bin, entities, self.uniform_index, self.phase_type);
        } else {
            context
                .alpha_phase
                .add(batch, bin, entities, self.uniform_index, self.phase_type);
        }
    }
}

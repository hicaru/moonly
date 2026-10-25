use bevy::{
    camera::primitives::{Aabb, MeshAabb},
    ecs::system::SystemParam,
    mesh::skinning::SkinnedMesh,
    pbr::MeshMaterial3d,
    prelude::*,
};
use moonly_core::{MoonlyConfig, MoonlyLight, MoonlyLightFlags};
use moonly_render::{MoonlyCamera, MoonlyCameraRole, MoonlyCasterVisibility};

use crate::{
    hom::MoonlyHomView,
    smap_vis::{SmapVisPhase, SmapVisTracker},
};

type CasterQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Mesh3d,
        &'static GlobalTransform,
        &'static MeshMaterial3d<StandardMaterial>,
    ),
    Without<SkinnedMesh>,
>;
type CameraQuery<'w, 's> = Query<'w, 's, (&'static MoonlyHomView, &'static MoonlyCamera)>;
type LightQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static MoonlyLight,
        Option<&'static mut SmapVisTracker>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct CasterInputs<'w, 's> {
    config: Res<'w, MoonlyConfig>,
    meshes: Option<Res<'w, Assets<Mesh>>>,
    materials: Option<Res<'w, Assets<StandardMaterial>>>,
    cameras: CameraQuery<'w, 's>,
    casters: CasterQuery<'w, 's>,
}

pub(crate) fn update_smapvis(
    mut commands: Commands,
    inputs: CasterInputs,
    mut lights: LightQuery,
    mut visibility: ResMut<MoonlyCasterVisibility>,
    mut frame: Local<u32>,
) {
    *frame = frame.wrapping_add(1);
    visibility.clear();
    let hom = active_hom(&inputs);
    for (entity, light, tracker) in &mut lights {
        if !light.flags.contains(MoonlyLightFlags::SHADOW) {
            continue;
        }
        let Some(mut tracker) = tracker else {
            commands.entity(entity).insert(SmapVisTracker::default());
            continue;
        };
        if hom.is_none() || !inputs.config.enable_smapvis {
            tracker.reset();
            continue;
        }
        if let Some(hom) = hom {
            advance_tracker(&mut tracker, hom, &inputs, *frame);
            for caster in &tracker.invisible {
                visibility.insert(entity, Entity::from_bits(*caster));
            }
        }
    }
}

fn active_hom<'a>(inputs: &'a CasterInputs<'_, '_>) -> Option<&'a MoonlyHomView> {
    if !inputs.config.enable_occ_rasterizer {
        return None;
    }
    inputs
        .cameras
        .iter()
        .find(|(hom, camera)| hom.enabled && camera.role == MoonlyCameraRole::World)
        .map(|(hom, _)| hom)
}

fn advance_tracker(
    tracker: &mut SmapVisTracker,
    hom: &MoonlyHomView,
    inputs: &CasterInputs,
    frame: u32,
) {
    if tracker.phase == SmapVisPhase::Scan {
        tracker.candidates.clear();
        tracker
            .candidates
            .extend(inputs.casters.iter().map(|(entity, ..)| entity.to_bits()));
    }
    if tracker.phase == SmapVisPhase::Test
        && let Some(caster) = tracker.candidates.get(tracker.current_test_index)
    {
        tracker.last_probe_fragments = u32::from(probe_caster(hom, inputs, *caster));
    }
    tracker.step_frame(frame);
}

fn probe_caster(hom: &MoonlyHomView, inputs: &CasterInputs, caster: u64) -> bool {
    let Ok((_, mesh, transform, material)) = inputs.casters.get(Entity::from_bits(caster)) else {
        return true;
    };
    let Some(material) = inputs
        .materials
        .as_ref()
        .and_then(|assets| assets.get(material))
    else {
        return true;
    };
    if !matches!(material.alpha_mode, AlphaMode::Opaque | AlphaMode::Mask(_)) {
        return true;
    }
    let Some(aabb) = inputs
        .meshes
        .as_ref()
        .and_then(|assets| assets.get(mesh))
        .and_then(MeshAabb::get_aabb)
    else {
        return true;
    };
    hom.aabb_visible(world_aabb(aabb, transform))
}

fn world_aabb(aabb: Aabb, transform: &GlobalTransform) -> Aabb {
    let matrix = transform.to_matrix();
    let center = matrix.transform_point3(aabb.center.into());
    let half = Vec3::from(aabb.half_extents);
    let row_x = Vec3::new(matrix.x_axis.x, matrix.y_axis.x, matrix.z_axis.x);
    let row_y = Vec3::new(matrix.x_axis.y, matrix.y_axis.y, matrix.z_axis.y);
    let row_z = Vec3::new(matrix.x_axis.z, matrix.y_axis.z, matrix.z_axis.z);
    let extent = Vec3::new(
        row_x.abs().dot(half),
        row_y.abs().dot(half),
        row_z.abs().dot(half),
    );
    Aabb::from_min_max(center - extent, center + extent)
}

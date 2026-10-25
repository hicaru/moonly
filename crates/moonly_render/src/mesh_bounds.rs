use std::collections::HashMap;

use bevy::{
    camera::primitives::{Aabb, MeshAabb},
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
};

#[derive(Resource, Debug, Default, Clone)]
pub(crate) struct MoonlyMeshBounds {
    values: HashMap<AssetId<Mesh>, Aabb>,
}

impl MoonlyMeshBounds {
    pub(crate) fn get(&self, mesh: AssetId<Mesh>) -> Option<Aabb> {
        self.values.get(&mesh).copied()
    }
}

impl ExtractResource<RenderApp> for MoonlyMeshBounds {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

pub(crate) fn collect_mesh_bounds(
    assets: Option<Res<Assets<Mesh>>>,
    mut bounds: ResMut<MoonlyMeshBounds>,
) {
    let Some(assets) = assets else {
        return;
    };
    if !assets.is_changed() {
        return;
    }
    bounds.values.clear();
    for (mesh_id, mesh) in assets.iter() {
        if let Some(aabb) = mesh.get_aabb() {
            bounds.values.insert(mesh_id, aabb);
        }
    }
}

pub(crate) fn sphere_visible(
    aabb: Aabb,
    transform: &GlobalTransform,
    clip_from_world: Mat4,
) -> bool {
    let matrix = transform.to_matrix();
    let scale = matrix
        .x_axis
        .truncate()
        .length()
        .max(matrix.y_axis.truncate().length())
        .max(matrix.z_axis.truncate().length());
    let center = matrix.transform_point3(aabb.center.into());
    let radius = aabb.half_extents.length() * scale;
    let clip = clip_from_world * center.extend(1.0);
    if clip.w <= 0.0 {
        return clip.w >= -radius;
    }
    let ndc = clip.xyz() / clip.w;
    let ratio = radius / clip.w.max(1.0e-6);
    ndc.x.abs() <= 1.0 + ratio
        && ndc.y.abs() <= 1.0 + ratio
        && ndc.z >= -ratio
        && ndc.z <= 1.0 + ratio
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_test_accepts_frustum_edge_and_rejects_outside_caster() {
        let aabb = Aabb::from_min_max(Vec3::splat(-0.5), Vec3::splat(0.5));
        let projection =
            moonly_core::perspective_rh_mat4(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 100.0);
        assert!(sphere_visible(
            aabb,
            &GlobalTransform::from(Transform::from_xyz(0.5, 0.0, -5.0)),
            projection,
        ));
        assert!(!sphere_visible(
            aabb,
            &GlobalTransform::from(Transform::from_xyz(20.0, 0.0, -5.0)),
            projection,
        ));
    }
}

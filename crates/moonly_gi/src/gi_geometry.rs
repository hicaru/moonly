use std::{
    hash::{BuildHasher, Hash, Hasher},
    sync::atomic::{AtomicU64, Ordering},
};

use bevy::{
    ecs::system::SystemParam,
    mesh::{Indices, VertexAttributeValues, skinning::SkinnedMesh},
    platform::hash::FixedHasher,
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

use crate::gi_bvh::{GiTriangle, GiTriangleBvh};
use crate::gi_ray::{GiRayGeometry, GiRayHit, GiRayQuery};
use moonly_core::HemisphereTracker;

/// Mark a static mesh entity for CPU GI ray intersection.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct MoonlyGiCaster {
    /// Reject triangles whose winding faces away from the ray.
    /// Moonly Monolith enables this behavior.
    pub cull_backfaces: bool,
}

impl Default for MoonlyGiCaster {
    fn default() -> Self {
        Self {
            cull_backfaces: true,
        }
    }
}

/// Cached static world-space geometry and revision for GI light caches.
#[derive(Resource, Debug, Default)]
pub struct StaticGiGeometry {
    bvh: GiTriangleBvh,
    revision: u64,
    caster_signature: u64,
    caster_count: usize,
    rays_traced: AtomicU64,
}

impl StaticGiGeometry {
    /// Return the current geometry revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Return the cumulative number of triangle BVH ray queries.
    #[must_use]
    pub fn rays_traced(&self) -> u64 {
        self.rays_traced.load(Ordering::Relaxed)
    }

    /// Return the triangle BVH.
    #[must_use]
    pub const fn bvh(&self) -> &GiTriangleBvh {
        &self.bvh
    }

    pub(crate) fn rebuild(&mut self, triangles: &mut Vec<GiTriangle>) {
        self.bvh.rebuild(triangles);
        self.revision = self.revision.wrapping_add(1);
    }
}

impl GiRayGeometry for StaticGiGeometry {
    fn nearest_hit(&self, ray: GiRayQuery) -> Option<GiRayHit> {
        self.rays_traced.fetch_add(1, Ordering::Relaxed);
        self.bvh.nearest_hit(ray)
    }

    fn any_hit(&self, ray: GiRayQuery) -> bool {
        self.rays_traced.fetch_add(1, Ordering::Relaxed);
        self.bvh.any_hit(ray)
    }
}

type CasterFilter = (Without<SkinnedMesh>, Without<HemisphereTracker>);
type ChangedCasterFilter = (
    Without<SkinnedMesh>,
    Without<HemisphereTracker>,
    Or<(
        Changed<Mesh3d>,
        Changed<GlobalTransform>,
        Changed<MoonlyGiCaster>,
    )>,
);
type CasterComponents = (
    Entity,
    &'static Mesh3d,
    &'static GlobalTransform,
    &'static MoonlyGiCaster,
);
type CasterQuery<'w, 's> = Query<'w, 's, CasterComponents, CasterFilter>;
type ChangedCasterQuery<'w, 's> = Query<'w, 's, (), ChangedCasterFilter>;

#[derive(SystemParam)]
pub(crate) struct GiGeometryInputs<'w, 's> {
    meshes: Option<Res<'w, Assets<Mesh>>>,
    casters: CasterQuery<'w, 's>,
    changed_casters: ChangedCasterQuery<'w, 's>,
}

/// Rebuild static triangle geometry after caster, transform, or mesh changes.
pub(crate) fn collect_static_gi_geometry(
    inputs: GiGeometryInputs,
    mut geometry: ResMut<StaticGiGeometry>,
    mut triangles: Local<Vec<GiTriangle>>,
) {
    let meshes_changed = inputs.meshes.as_ref().is_some_and(Res::is_changed);
    let current_count = inputs.casters.iter().count();
    if geometry.revision != 0
        && !meshes_changed
        && inputs.changed_casters.is_empty()
        && current_count == geometry.caster_count
    {
        return;
    }
    let signature = caster_signature(&inputs.casters, inputs.meshes.as_deref());
    if geometry.revision != 0 && signature == geometry.caster_signature {
        geometry.caster_count = current_count;
        return;
    }
    triangles.clear();
    if let Some(meshes) = inputs.meshes.as_ref() {
        for (_, mesh, transform, caster) in &inputs.casters {
            if let Some(mesh) = meshes.get(mesh) {
                append_mesh_triangles(mesh, transform, caster.cull_backfaces, &mut triangles);
            }
        }
    }
    geometry.rebuild(&mut triangles);
    geometry.caster_signature = signature;
    geometry.caster_count = current_count;
}

fn caster_signature(casters: &CasterQuery, meshes: Option<&Assets<Mesh>>) -> u64 {
    casters
        .iter()
        .fold(0, |combined, (entity, mesh, transform, caster)| {
            let mut hasher = FixedHasher.build_hasher();
            entity.hash(&mut hasher);
            mesh.id().hash(&mut hasher);
            caster.cull_backfaces.hash(&mut hasher);
            let cols = transform.to_matrix().to_cols_array();
            for value in cols {
                hasher.write_u32(value.to_bits());
            }
            if let Some(asset) = meshes.and_then(|meshes| meshes.get(mesh)) {
                hash_mesh_geometry(asset, &mut hasher);
            }
            combined ^ hasher.finish()
        })
}

#[expect(
    unsafe_code,
    reason = "`[f32; 3]` has 12 contiguous initialized bytes with no padding."
)]
fn hash_mesh_geometry(mesh: &Mesh, hasher: &mut impl Hasher) {
    std::mem::discriminant(&mesh.primitive_topology()).hash(hasher);
    let attribute = mesh.attribute(Mesh::ATTRIBUTE_POSITION);
    std::mem::discriminant(&attribute).hash(hasher);
    if let Some(values) = attribute {
        std::mem::discriminant(values).hash(hasher);
    }
    if let Some(VertexAttributeValues::Float32x3(positions)) = attribute {
        hasher.write_usize(positions.len());
        // SAFETY: `[f32; 3]` is 12 contiguous initialized bytes with no padding.
        let bytes = unsafe {
            std::slice::from_raw_parts(positions.as_ptr().cast::<u8>(), positions.len() * 12)
        };
        hasher.write(bytes);
    }
    match mesh.indices() {
        Some(Indices::U16(indices)) => indices.hash(hasher),
        Some(Indices::U32(indices)) => indices.hash(hasher),
        None => mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .map_or(0, VertexAttributeValues::len)
            .hash(hasher),
    }
}

pub(crate) fn append_mesh_triangles(
    mesh: &Mesh,
    transform: &GlobalTransform,
    cull_backfaces: bool,
    out: &mut Vec<GiTriangle>,
) {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    let topology = mesh.primitive_topology();
    if !matches!(
        topology,
        PrimitiveTopology::TriangleList | PrimitiveTopology::TriangleStrip
    ) {
        return;
    }
    let vertex_count = mesh.indices().map_or(positions.len(), Indices::len);
    let triangle_count = match topology {
        PrimitiveTopology::TriangleList => vertex_count / 3,
        PrimitiveTopology::TriangleStrip => vertex_count.saturating_sub(2),
        _ => return,
    };
    for triangle in 0..triangle_count {
        let Some(vertices) = mesh_triangle(mesh, positions, triangle, topology) else {
            continue;
        };
        let world = vertices.map(|vertex| transform.transform_point(Vec3::from_array(vertex)));
        if let Some(triangle) = GiTriangle::new(world, cull_backfaces) {
            out.push(triangle);
        }
    }
}

fn mesh_triangle(
    mesh: &Mesh,
    positions: &[[f32; 3]],
    triangle: usize,
    topology: PrimitiveTopology,
) -> Option<[[f32; 3]; 3]> {
    let base = match topology {
        PrimitiveTopology::TriangleList => triangle.checked_mul(3)?,
        PrimitiveTopology::TriangleStrip => triangle,
        _ => return None,
    };
    let mut indices = [base, base.checked_add(1)?, base.checked_add(2)?];
    if topology == PrimitiveTopology::TriangleStrip && triangle % 2 == 1 {
        indices.swap(0, 1);
    }
    let mut vertices = [[0.0; 3]; 3];
    for (corner, index) in indices.into_iter().enumerate() {
        let index = mesh_vertex_index(mesh, index)?;
        *vertices.get_mut(corner)? = *positions.get(index)?;
    }
    Some(vertices)
}

fn mesh_vertex_index(mesh: &Mesh, index: usize) -> Option<usize> {
    match mesh.indices() {
        Some(Indices::U16(indices)) => indices.get(index).map(|index| *index as usize),
        Some(Indices::U32(indices)) => indices.get(index).map(|index| *index as usize),
        None => Some(index),
    }
}

#[cfg(test)]
mod tests;

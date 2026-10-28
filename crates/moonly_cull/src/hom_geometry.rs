use std::hash::{Hash, Hasher};

use bevy::{
    mesh::{Indices, VertexAttributeValues, skinning::SkinnedMesh},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct MoonlyHomOccluder {
    pub double_sided: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct HomTriangle {
    pub vertices: [Vec3; 3],
    pub double_sided: bool,
}

#[derive(Resource, Default)]
pub(crate) struct MoonlyHomGeometry {
    pub triangles: Vec<HomTriangle>,
    signature: u64,
    revision: u64,
    initialized: bool,
}

impl MoonlyHomGeometry {
    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }
}

type OccluderQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Mesh3d,
        &'static GlobalTransform,
        &'static MoonlyHomOccluder,
    ),
    Without<SkinnedMesh>,
>;

pub(crate) fn sync_hom_geometry(
    assets: Option<Res<Assets<Mesh>>>,
    occluders: OccluderQuery,
    mut geometry: ResMut<MoonlyHomGeometry>,
) {
    let signature = geometry_signature(&occluders);
    let assets_changed = assets.as_ref().is_some_and(Res::is_changed);
    if geometry.initialized && geometry.signature == signature && !assets_changed {
        return;
    }
    geometry.triangles.clear();
    if let Some(assets) = assets {
        for (_, mesh, transform, occluder) in &occluders {
            if let Some(mesh) = assets.get(mesh) {
                append_mesh(
                    mesh,
                    transform,
                    occluder.double_sided,
                    &mut geometry.triangles,
                );
            }
        }
    }
    geometry.signature = signature;
    geometry.revision = geometry.revision.wrapping_add(1);
    geometry.initialized = true;
}

fn geometry_signature(occluders: &OccluderQuery) -> u64 {
    occluders
        .iter()
        .fold(0, |combined, (entity, mesh, transform, occluder)| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            entity.hash(&mut hasher);
            mesh.id().hash(&mut hasher);
            occluder.double_sided.hash(&mut hasher);
            for value in transform.to_matrix().to_cols_array() {
                value.to_bits().hash(&mut hasher);
            }
            combined ^ hasher.finish()
        })
}

fn append_mesh(
    mesh: &Mesh,
    transform: &GlobalTransform,
    double_sided: bool,
    output: &mut Vec<HomTriangle>,
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
    let count = mesh.indices().map_or(positions.len(), Indices::len);
    let triangles = match topology {
        PrimitiveTopology::TriangleList => count / 3,
        PrimitiveTopology::TriangleStrip => count.saturating_sub(2),
        _ => return,
    };
    for triangle in 0..triangles {
        let mut indices = [triangle * 3, triangle * 3 + 1, triangle * 3 + 2];
        if topology == PrimitiveTopology::TriangleStrip {
            indices = [triangle, triangle + 1, triangle + 2];
            if triangle % 2 == 1 {
                indices.swap(0, 1);
            }
        }
        let Some(vertices) = mesh_vertices(mesh, positions, indices, transform) else {
            continue;
        };
        output.push(HomTriangle {
            vertices,
            double_sided,
        });
    }
}

fn mesh_vertices(
    mesh: &Mesh,
    positions: &[[f32; 3]],
    indices: [usize; 3],
    transform: &GlobalTransform,
) -> Option<[Vec3; 3]> {
    let mut vertices = [Vec3::ZERO; 3];
    for (corner, index) in indices.into_iter().enumerate() {
        let index = mesh_vertex_index(mesh, index)?;
        let position = positions.get(index)?;
        *vertices.get_mut(corner)? = transform.transform_point(Vec3::from_array(*position));
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

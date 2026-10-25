use bevy::prelude::*;

use crate::gi_intersection::intersect_ray_aabb;
use crate::gi_ray::{GiRayGeometry, GiRayHit, GiRayQuery};

const MOONLY_TRIANGLE_EPSILON: f32 = 0.000_01;
const BVH_LEAF_TRIANGLES: usize = 4;
const NO_CHILD: u32 = u32::MAX;

/// Static world-space triangle prepared for GI traversal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GiTriangle {
    pub vertices: [Vec3; 3],
    pub normal: Vec3,
    pub cull_backfaces: bool,
}

impl GiTriangle {
    /// Create a valid triangle and compute its geometric winding normal.
    #[must_use]
    pub fn new(vertices: [Vec3; 3], cull_backfaces: bool) -> Option<Self> {
        let normal = (vertices.get(1)? - vertices.first()?)
            .cross(vertices.get(2)? - vertices.first()?)
            .normalize_or_zero();
        if normal == Vec3::ZERO || vertices.iter().any(|vertex| !vertex.is_finite()) {
            return None;
        }
        Some(Self {
            vertices,
            normal,
            cull_backfaces,
        })
    }

    fn bounds(&self) -> GiBounds {
        self.vertices
            .iter()
            .fold(GiBounds::EMPTY, |bounds, vertex| bounds.include(*vertex))
    }

    fn centroid(&self) -> Vec3 {
        self.vertices.iter().copied().sum::<Vec3>() / 3.0
    }
}

#[derive(Debug, Clone, Copy)]
struct GiBounds {
    min: Vec3,
    max: Vec3,
}

impl GiBounds {
    const EMPTY: Self = Self {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };

    fn include(mut self, point: Vec3) -> Self {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
        self
    }

    fn join(self, other: Self) -> Self {
        self.include(other.min).include(other.max)
    }
}

#[derive(Debug, Clone, Copy)]
struct BvhNode {
    bounds: GiBounds,
    left: u32,
    right: u32,
    first_triangle: u32,
    triangle_count: u32,
}

impl BvhNode {
    const fn leaf(bounds: GiBounds, first_triangle: usize, triangle_count: usize) -> Self {
        Self {
            bounds,
            left: NO_CHILD,
            right: NO_CHILD,
            first_triangle: first_triangle as u32,
            triangle_count: triangle_count as u32,
        }
    }
}

/// World-space static triangle BVH for Moonly-compatible nearest-hit rays.
#[derive(Debug, Default, Clone)]
pub struct GiTriangleBvh {
    triangles: Vec<GiTriangle>,
    nodes: Vec<BvhNode>,
}

impl GiTriangleBvh {
    /// Replace all triangles and rebuild the acceleration structure.
    pub fn rebuild(&mut self, triangles: &mut Vec<GiTriangle>) {
        self.triangles.clear();
        self.nodes.clear();
        std::mem::swap(&mut self.triangles, triangles);
        if !self.triangles.is_empty() {
            build_node(&mut self.triangles, 0, &mut self.nodes);
        }
    }

    /// Return the number of triangles in the acceleration structure.
    #[must_use]
    pub const fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Return the number of BVH nodes.
    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Return allocated CPU bytes for triangles and nodes.
    #[must_use]
    pub const fn allocated_bytes(&self) -> usize {
        self.triangles.capacity() * size_of::<GiTriangle>()
            + self.nodes.capacity() * size_of::<BvhNode>()
    }

    fn traverse(&self, node_index: u32, ray: GiRayQuery, best: &mut Option<GiRayHit>) {
        let Ok(node_index) = usize::try_from(node_index) else {
            return;
        };
        let Some(node) = self.nodes.get(node_index) else {
            return;
        };
        let max_distance = best.map_or(ray.max_distance, |hit| hit.distance);
        let Some((distance, _)) =
            intersect_ray_aabb(ray.origin, ray.direction, node.bounds.min, node.bounds.max)
        else {
            return;
        };
        if distance > max_distance {
            return;
        }
        if node.left == NO_CHILD {
            self.intersect_leaf(node, ray, best);
            return;
        }
        self.traverse(node.left, ray, best);
        self.traverse(node.right, ray, best);
    }

    fn intersect_leaf(&self, node: &BvhNode, ray: GiRayQuery, best: &mut Option<GiRayHit>) {
        let start = node.first_triangle as usize;
        let end = start.saturating_add(node.triangle_count as usize);
        let Some(triangles) = self.triangles.get(start..end) else {
            return;
        };
        for triangle in triangles {
            let max_distance = best.map_or(ray.max_distance, |hit| hit.distance);
            if let Some(distance) = intersect_triangle(triangle, ray, max_distance) {
                *best = Some(GiRayHit {
                    distance,
                    normal: triangle.normal,
                    albedo: Vec3::ONE,
                });
            }
        }
    }
}

impl GiRayGeometry for GiTriangleBvh {
    fn nearest_hit(&self, ray: GiRayQuery) -> Option<GiRayHit> {
        if !valid_ray(ray) {
            return None;
        }
        let mut best = None;
        self.traverse(0, ray, &mut best);
        best
    }
}

fn valid_ray(ray: GiRayQuery) -> bool {
    ray.origin.is_finite()
        && ray.direction.is_finite()
        && ray.direction != Vec3::ZERO
        && ray.min_distance.is_finite()
        && ray.max_distance.is_finite()
        && ray.max_distance > 0.0
}

fn build_node(triangles: &mut [GiTriangle], first: usize, nodes: &mut Vec<BvhNode>) -> u32 {
    let bounds = triangles
        .iter()
        .fold(GiBounds::EMPTY, |bounds, tri| bounds.join(tri.bounds()));
    let node_index = nodes.len() as u32;
    if triangles.len() <= BVH_LEAF_TRIANGLES {
        nodes.push(BvhNode::leaf(bounds, first, triangles.len()));
        return node_index;
    }
    nodes.push(BvhNode::leaf(bounds, first, 0));
    let extent = bounds.max - bounds.min;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    triangles.sort_unstable_by(|a, b| {
        axis_value(a.centroid(), axis).total_cmp(&axis_value(b.centroid(), axis))
    });
    let middle = triangles.len() / 2;
    let Some((left, right)) = triangles.split_at_mut_checked(middle) else {
        return node_index;
    };
    let left_index = build_node(left, first, nodes);
    let right_index = build_node(right, first + middle, nodes);
    if let Some(node) = nodes.get_mut(node_index as usize) {
        node.left = left_index;
        node.right = right_index;
    }
    node_index
}

const fn axis_value(value: Vec3, axis: usize) -> f32 {
    match axis {
        0 => value.x,
        1 => value.y,
        _ => value.z,
    }
}

fn intersect_triangle(triangle: &GiTriangle, ray: GiRayQuery, max_distance: f32) -> Option<f32> {
    let first = triangle.vertices.first()?;
    let edge1 = triangle.vertices.get(1)? - first;
    let edge2 = triangle.vertices.get(2)? - first;
    let pvec = ray.direction.cross(edge2);
    let determinant = edge1.dot(pvec);
    if (triangle.cull_backfaces && determinant < MOONLY_TRIANGLE_EPSILON)
        || (!triangle.cull_backfaces && determinant.abs() < MOONLY_TRIANGLE_EPSILON)
    {
        return None;
    }
    let inverse = determinant.recip();
    let tvec = ray.origin - first;
    let u = tvec.dot(pvec) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qvec = tvec.cross(edge1);
    let v = ray.direction.dot(qvec) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = edge2.dot(qvec) * inverse;
    (distance > 0.0 && distance >= ray.min_distance && distance < max_distance).then_some(distance)
}

#[cfg(test)]
mod tests;

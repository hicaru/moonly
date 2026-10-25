use bevy::{math::Vec3A, prelude::*};

const MOONLY_TRIANGLE_EPSILON: f32 = 0.000_01;
const BVH_LEAF_TRIANGLES: usize = 4;
const LEAF_FLAG: u32 = 0x8000_0000;
const SPLIT_EXTENT_EPSILON: f32 = 1.0e-5;

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
        let [v0, v1, v2] = vertices;
        let normal = (v1 - v0).cross(v2 - v0).normalize_or_zero();
        if normal == Vec3::ZERO || !v0.is_finite() || !v1.is_finite() || !v2.is_finite() {
            return None;
        }
        Some(Self {
            vertices,
            normal,
            cull_backfaces,
        })
    }

    #[inline]
    fn simd_bounds_and_sum3(&self) -> (Vec3A, Vec3A, Vec3A) {
        let [a, b, c] = self.vertices;
        let (v0, v1, v2) = (Vec3A::from(a), Vec3A::from(b), Vec3A::from(c));
        (v0.min(v1).min(v2), v0.max(v1).max(v2), v0 + v1 + v2)
    }

    #[inline]
    fn axis_sum3(&self, axis: usize) -> f32 {
        let [v0, v1, v2] = self.vertices;
        match axis {
            0 => v0.x + v1.x + v2.x,
            1 => v0.y + v1.y + v2.y,
            _ => v0.z + v1.z + v2.z,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct NodeBounds {
    bmin: Vec3A,
    bmax: Vec3A,
    cmin: Vec3A,
    cmax: Vec3A,
}

impl NodeBounds {
    const EMPTY: Self = Self {
        bmin: Vec3A::splat(f32::INFINITY),
        bmax: Vec3A::splat(f32::NEG_INFINITY),
        cmin: Vec3A::splat(f32::INFINITY),
        cmax: Vec3A::splat(f32::NEG_INFINITY),
    };

    #[inline]
    fn include(&mut self, tri: &GiTriangle) {
        let (tmin, tmax, sum3) = tri.simd_bounds_and_sum3();
        self.bmin = self.bmin.min(tmin);
        self.bmax = self.bmax.max(tmax);
        self.cmin = self.cmin.min(sum3);
        self.cmax = self.cmax.max(sum3);
    }

    #[inline]
    fn from_slice(triangles: &[GiTriangle]) -> Self {
        let mut stats = Self::EMPTY;
        for tri in triangles {
            stats.include(tri);
        }
        stats
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct BvhNode {
    min: Vec3,
    left_or_first: u32,
    max: Vec3,
    right_or_count: u32,
}

impl BvhNode {
    #[inline]
    fn leaf(stats: &NodeBounds, first_triangle: usize, triangle_count: usize) -> Self {
        Self {
            min: Vec3::from(stats.bmin),
            left_or_first: first_triangle as u32,
            max: Vec3::from(stats.bmax),
            right_or_count: LEAF_FLAG | (triangle_count as u32),
        }
    }

    #[inline]
    const fn is_leaf(self) -> bool {
        (self.right_or_count & LEAF_FLAG) != 0
    }

    #[inline]
    const fn triangle_count(self) -> usize {
        (self.right_or_count & !LEAF_FLAG) as usize
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
            let estimated_nodes = (self.triangles.len() / 2).saturating_add(1);
            if self.nodes.capacity() < estimated_nodes {
                self.nodes.reserve(estimated_nodes - self.nodes.len());
            }
            let root_stats = NodeBounds::from_slice(&self.triangles);
            build_node(&mut self.triangles, 0, root_stats, &mut self.nodes);
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
}

fn build_node(
    triangles: &mut [GiTriangle],
    first: usize,
    stats: NodeBounds,
    nodes: &mut Vec<BvhNode>,
) -> u32 {
    let node_index = nodes.len() as u32;
    nodes.push(BvhNode::leaf(&stats, first, triangles.len()));
    if triangles.len() <= BVH_LEAF_TRIANGLES {
        return node_index;
    }
    let c_ext = stats.cmax - stats.cmin;
    let axis = if c_ext.x >= c_ext.y && c_ext.x >= c_ext.z {
        0
    } else if c_ext.y >= c_ext.z {
        1
    } else {
        2
    };
    let span = [c_ext.x, c_ext.y, c_ext.z]
        .get(axis)
        .copied()
        .unwrap_or(0.0);
    let (mid, left_stats, right_stats) = if span > SPLIT_EXTENT_EPSILON {
        let sum = [
            stats.cmin.x + stats.cmax.x,
            stats.cmin.y + stats.cmax.y,
            stats.cmin.z + stats.cmax.z,
        ];
        partition_midpoint(triangles, axis, 0.5 * sum.get(axis).copied().unwrap_or(0.0))
    } else {
        partition_median_fallback(triangles, axis)
    };
    let Some((left, right)) = triangles.split_at_mut_checked(mid) else {
        return node_index;
    };
    let left_index = build_node(left, first, left_stats, nodes);
    let right_index = build_node(right, first + mid, right_stats, nodes);
    if let Some(node) = nodes.get_mut(node_index as usize) {
        node.left_or_first = left_index;
        node.right_or_count = right_index;
    }
    node_index
}

#[inline]
fn partition_midpoint(
    triangles: &mut [GiTriangle],
    axis: usize,
    split_val: f32,
) -> (usize, NodeBounds, NodeBounds) {
    let mut left_stats = NodeBounds::EMPTY;
    let mut right_stats = NodeBounds::EMPTY;
    let mut split = 0_usize;
    for i in 0..triangles.len() {
        let Some(&tri) = triangles.get(i) else {
            break;
        };
        if tri.axis_sum3(axis) < split_val {
            left_stats.include(&tri);
            triangles.swap(i, split);
            split += 1;
        } else {
            right_stats.include(&tri);
        }
    }
    if split == 0 || split == triangles.len() {
        return partition_median_fallback(triangles, axis);
    }
    (split, left_stats, right_stats)
}

#[inline]
fn partition_median_fallback(
    triangles: &mut [GiTriangle],
    axis: usize,
) -> (usize, NodeBounds, NodeBounds) {
    let mid = triangles.len() / 2;
    let _ = triangles
        .select_nth_unstable_by(mid, |a, b| a.axis_sum3(axis).total_cmp(&b.axis_sum3(axis)));
    let (left, right) = triangles.split_at(mid);
    (
        mid,
        NodeBounds::from_slice(left),
        NodeBounds::from_slice(right),
    )
}

#[cfg(test)]
use crate::gi_ray::{GiRayGeometry, GiRayQuery};
#[cfg(test)]
mod tests;
mod traverse;

use bevy::{math::Vec3A, prelude::*};

use super::{BvhNode, GiTriangle, GiTriangleBvh, MOONLY_TRIANGLE_EPSILON};
use crate::gi_intersection::PrecomputedRay;
use crate::gi_ray::{GiRayGeometry, GiRayHit, GiRayQuery};

struct TraversalState {
    ray: GiRayQuery,
    pre: PrecomputedRay,
    best_dist: f32,
    best_hit: Option<GiRayHit>,
    stack: [(u32, f32); 64],
    stack_len: usize,
    stop_first: bool,
}

impl TraversalState {
    #[inline]
    fn pop_valid(&mut self) -> Option<u32> {
        while self.stack_len > 0 {
            self.stack_len -= 1;
            if let Some(&(idx, entry_t)) = self.stack.get(self.stack_len)
                && entry_t <= self.best_dist
            {
                return Some(idx);
            }
        }
        None
    }
}

impl GiTriangleBvh {
    fn traverse(&self, ray: GiRayQuery, pre: PrecomputedRay, stop_first: bool) -> Option<GiRayHit> {
        let root = self.nodes.first()?;
        pre.hit_bvh_aabb(
            Vec3A::from(root.min),
            Vec3A::from(root.max),
            ray.max_distance,
        )?;
        let mut st = TraversalState {
            ray,
            pre,
            best_dist: ray.max_distance,
            best_hit: None,
            stack: [(0, 0.0); 64],
            stack_len: 0,
            stop_first,
        };
        let mut current = 0_u32;
        loop {
            if let Some(&node) = self.nodes.get(current as usize) {
                if node.is_leaf() {
                    if self.intersect_leaf(&node, &mut st) {
                        return st.best_hit;
                    }
                } else if let Some(next) = self.step_children(&node, &mut st) {
                    current = next;
                    continue;
                }
            }
            let Some(next) = st.pop_valid() else {
                break;
            };
            current = next;
        }
        st.best_hit
    }

    #[inline]
    fn step_children(&self, node: &BvhNode, st: &mut TraversalState) -> Option<u32> {
        let (left_idx, right_idx) = (node.left_or_first, node.right_or_count);
        let left_n = self.nodes.get(left_idx as usize)?;
        let right_n = self.nodes.get(right_idx as usize)?;
        let hit_l = st.pre.hit_bvh_aabb(
            Vec3A::from(left_n.min),
            Vec3A::from(left_n.max),
            st.best_dist,
        );
        let hit_r = st.pre.hit_bvh_aabb(
            Vec3A::from(right_n.min),
            Vec3A::from(right_n.max),
            st.best_dist,
        );
        match (hit_l, hit_r) {
            (Some(tl), Some(tr)) => {
                let (near, far, far_t) = if tl <= tr {
                    (left_idx, right_idx, tr)
                } else {
                    (right_idx, left_idx, tl)
                };
                if let Some(slot) = st.stack.get_mut(st.stack_len) {
                    *slot = (far, far_t);
                    st.stack_len += 1;
                }
                Some(near)
            }
            (Some(_), None) => Some(left_idx),
            (None, Some(_)) => Some(right_idx),
            (None, None) => None,
        }
    }

    #[inline]
    fn intersect_leaf(&self, node: &BvhNode, st: &mut TraversalState) -> bool {
        let start = node.left_or_first as usize;
        let Some(triangles) = self
            .triangles
            .get(start..start.saturating_add(node.triangle_count()))
        else {
            return false;
        };
        for triangle in triangles {
            if let Some(distance) = intersect_triangle(triangle, st.ray, st.best_dist) {
                st.best_dist = distance;
                st.best_hit = Some(GiRayHit {
                    distance,
                    normal: triangle.normal,
                    albedo: Vec3::ONE,
                });
                if st.stop_first {
                    return true;
                }
            }
        }
        false
    }
}

impl GiRayGeometry for GiTriangleBvh {
    fn nearest_hit(&self, ray: GiRayQuery) -> Option<GiRayHit> {
        valid_ray(ray)
            .then(|| PrecomputedRay::new(ray.origin, ray.direction))
            .flatten()
            .and_then(|pre| self.traverse(ray, pre, false))
    }

    fn any_hit(&self, ray: GiRayQuery) -> bool {
        valid_ray(ray)
            && PrecomputedRay::new(ray.origin, ray.direction)
                .is_some_and(|pre| self.traverse(ray, pre, true).is_some())
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

#[inline]
fn intersect_triangle(triangle: &GiTriangle, ray: GiRayQuery, max_distance: f32) -> Option<f32> {
    let [v0, v1, v2] = triangle.vertices;
    let (edge1, edge2) = (v1 - v0, v2 - v0);
    let pvec = ray.direction.cross(edge2);
    let det = edge1.dot(pvec);
    if triangle.cull_backfaces {
        if det < MOONLY_TRIANGLE_EPSILON {
            return None;
        }
        let tvec = ray.origin - v0;
        let u_det = tvec.dot(pvec);
        if u_det < 0.0 || u_det > det {
            return None;
        }
        let qvec = tvec.cross(edge1);
        let v_det = ray.direction.dot(qvec);
        if v_det < 0.0 || u_det + v_det > det {
            return None;
        }
        let distance = edge2.dot(qvec) * det.recip();
        return (distance > 0.0 && distance >= ray.min_distance && distance < max_distance)
            .then_some(distance);
    }
    if det.abs() < MOONLY_TRIANGLE_EPSILON {
        return None;
    }
    let inverse = det.recip();
    let tvec = ray.origin - v0;
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

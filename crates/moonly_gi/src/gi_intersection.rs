use bevy::{
    math::{BVec3A, Vec3A},
    prelude::*,
};

use crate::gi_bounce::GiReflectorBox;

/// Precomputed SIMD ray parameters for fast repeated AABB slab tests.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PrecomputedRay {
    pub origin: Vec3A,
    pub inv_dir: Vec3A,
    pub parallel_origin_lo: Vec3A,
    pub parallel_origin_hi: Vec3A,
    pub is_parallel: BVec3A,
    pub has_parallel: bool,
}

impl PrecomputedRay {
    #[inline]
    #[must_use]
    pub(crate) fn new(origin: Vec3, dir: Vec3) -> Option<Self> {
        if !origin.is_finite() || !dir.is_finite() {
            return None;
        }
        let origin_a = Vec3A::from(origin);
        let dir_a = Vec3A::from(dir);
        let is_parallel = dir_a.cmpeq(Vec3A::ZERO);
        if is_parallel.all() {
            return None;
        }
        let has_parallel = is_parallel.any();
        let safe_dir = Vec3A::select(is_parallel, Vec3A::ONE, dir_a);
        let inv_dir = safe_dir.recip();
        let parallel_origin_lo = Vec3A::select(is_parallel, origin_a, Vec3A::splat(f32::INFINITY));
        let parallel_origin_hi =
            Vec3A::select(is_parallel, origin_a, Vec3A::splat(f32::NEG_INFINITY));
        Some(Self {
            origin: origin_a,
            inv_dir,
            parallel_origin_lo,
            parallel_origin_hi,
            is_parallel,
            has_parallel,
        })
    }

    #[inline]
    #[must_use]
    pub(crate) fn slab_t_fast(&self, min: Vec3A, max: Vec3A) -> Option<(f32, f32)> {
        let t1 = (min - self.origin) * self.inv_dir;
        let t2 = (max - self.origin) * self.inv_dir;
        let t_near = t1.min(t2).max_element();
        let t_far = t1.max(t2).min_element();
        (t_near <= t_far && t_far >= 0.0).then_some((t_near, t_far))
    }

    #[inline]
    #[must_use]
    pub(crate) fn slab_t(&self, min: Vec3A, max: Vec3A) -> Option<(f32, f32)> {
        if !self.has_parallel {
            return self.slab_t_fast(min, max);
        }
        if min.cmpgt(self.parallel_origin_lo).any() || max.cmplt(self.parallel_origin_hi).any() {
            return None;
        }
        let t1 = (min - self.origin) * self.inv_dir;
        let t2 = (max - self.origin) * self.inv_dir;
        let t_min = Vec3A::select(
            self.is_parallel,
            Vec3A::splat(f32::NEG_INFINITY),
            t1.min(t2),
        );
        let t_max = Vec3A::select(self.is_parallel, Vec3A::splat(f32::INFINITY), t1.max(t2));
        let t_near = t_min.max_element();
        let t_far = t_max.min_element();
        (t_near <= t_far && t_far >= 0.0).then_some((t_near, t_far))
    }

    #[inline]
    #[must_use]
    pub(crate) fn hit_bvh_aabb(&self, min: Vec3A, max: Vec3A, max_distance: f32) -> Option<f32> {
        let (t_near, t_far) = self.slab_t(min, max)?;
        let hit_t = if t_near >= 0.0 { t_near } else { t_far };
        (hit_t.is_finite() && hit_t <= max_distance).then_some(t_near.max(0.0))
    }
}

#[inline]
fn valid_aabb_simd(min_v: Vec3A, max_v: Vec3A) -> bool {
    min_v.cmple(max_v).all() && min_v.is_finite() && max_v.is_finite()
}

pub(crate) fn raycast_all_reflectors(
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
    min_dist: f32,
    reflectors: &[GiReflectorBox],
) -> Option<(f32, Vec3, Vec3)> {
    if !max_dist.is_finite() || max_dist <= 0.0 || !min_dist.is_finite() {
        return None;
    }
    let pre = PrecomputedRay::new(origin, dir)?;
    let min_clamped = min_dist.max(0.0);
    let mut best_t = max_dist;
    let mut best_box: Option<&GiReflectorBox> = None;
    if pre.has_parallel {
        for reflector in reflectors {
            let min_v = Vec3A::from(reflector.min);
            let max_v = Vec3A::from(reflector.max);
            if let Some((t_near, t_far)) = pre.slab_t(min_v, max_v) {
                let t = if t_near >= 0.0 { t_near } else { t_far };
                if t >= min_clamped && t < best_t && t.is_finite() && valid_aabb_simd(min_v, max_v)
                {
                    best_t = t;
                    best_box = Some(reflector);
                }
            }
        }
    } else {
        for reflector in reflectors {
            let min_v = Vec3A::from(reflector.min);
            let max_v = Vec3A::from(reflector.max);
            if let Some((t_near, t_far)) = pre.slab_t_fast(min_v, max_v) {
                let t = if t_near >= 0.0 { t_near } else { t_far };
                if t >= min_clamped && t < best_t && t.is_finite() && valid_aabb_simd(min_v, max_v)
                {
                    best_t = t;
                    best_box = Some(reflector);
                }
            }
        }
    }
    let winner = best_box?;
    let (t, normal) = intersect_ray_aabb(origin, dir, winner.min, winner.max)?;
    Some((t, normal, winner.albedo))
}

pub(crate) fn raycast_any_reflector(
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
    min_dist: f32,
    reflectors: &[GiReflectorBox],
) -> bool {
    if !max_dist.is_finite() || max_dist <= 0.0 || !min_dist.is_finite() {
        return false;
    }
    let Some(pre) = PrecomputedRay::new(origin, dir) else {
        return false;
    };
    let min_clamped = min_dist.max(0.0);
    for reflector in reflectors {
        let min_v = Vec3A::from(reflector.min);
        let max_v = Vec3A::from(reflector.max);
        if let Some((t_near, t_far)) = pre.slab_t(min_v, max_v) {
            let t = if t_near >= 0.0 { t_near } else { t_far };
            if t >= min_clamped && t < max_dist && t.is_finite() && valid_aabb_simd(min_v, max_v) {
                return true;
            }
        }
    }
    false
}

pub(crate) fn intersect_ray_aabb(
    origin: Vec3,
    dir: Vec3,
    min: Vec3,
    max: Vec3,
) -> Option<(f32, Vec3)> {
    if !origin.is_finite()
        || !dir.is_finite()
        || !min.is_finite()
        || !max.is_finite()
        || !min.cmple(max).all()
    {
        return None;
    }
    let origins = [origin.x, origin.y, origin.z];
    let directions = [dir.x, dir.y, dir.z];
    let minima = [min.x, min.y, min.z];
    let maxima = [max.x, max.y, max.z];
    let mut entry = (f32::NEG_INFINITY, 0_usize, 0.0_f32);
    let mut exit = (f32::INFINITY, 0_usize, 0.0_f32);
    for axis in 0..3 {
        let (Some(&o), Some(&d), Some(&lo), Some(&hi)) = (
            origins.get(axis),
            directions.get(axis),
            minima.get(axis),
            maxima.get(axis),
        ) else {
            return None;
        };
        if d == 0.0 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let mut near = ((lo - o) / d, -1.0_f32);
        let mut far = ((hi - o) / d, 1.0_f32);
        if near.0 > far.0 {
            std::mem::swap(&mut near, &mut far);
        }
        if near.0 > entry.0 {
            entry = (near.0, axis, near.1);
        }
        if far.0 < exit.0 {
            exit = (far.0, axis, far.1);
        }
        if entry.0 > exit.0 {
            return None;
        }
    }
    let (distance, axis, sign) = if entry.0 >= 0.0 { entry } else { exit };
    if exit.0 < 0.0 || !distance.is_finite() {
        return None;
    }
    Some((distance, axis_normal(axis, sign)))
}

const fn axis_normal(axis: usize, sign: f32) -> Vec3 {
    match axis {
        0 => Vec3::new(sign, 0.0, 0.0),
        1 => Vec3::new(0.0, sign, 0.0),
        _ => Vec3::new(0.0, 0.0, sign),
    }
}

#[cfg(test)]
mod tests;

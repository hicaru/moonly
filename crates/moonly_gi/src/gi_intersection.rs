use bevy::prelude::*;

use crate::gi_bounce::GiReflectorBox;

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
    let mut best_t = max_dist;
    let mut best_hit = None;
    for reflector in reflectors {
        if let Some((t, normal)) = intersect_ray_aabb(origin, dir, reflector.min, reflector.max)
            && t >= min_dist.max(0.0)
            && t < best_t
        {
            best_t = t;
            best_hit = Some((t, normal, reflector.albedo));
        }
    }
    best_hit
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

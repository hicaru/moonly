//! Virtual GI bounce generation with Monolith and enhanced reflector models.

use bevy::prelude::*;
use moonly_core::{
    GiRadiosityConfig, MoonlyLight, MoonlyLightFlags, MoonlyLightType, VirtualGiLight,
};

pub(crate) use crate::gi_intersection::raycast_all_reflectors;
use crate::gi_range::reference_range;
use crate::gi_ray::{GiRayGeometry, GiRayQuery};
use crate::gi_sampling::{EnhancedLightBasis, MONOLITH_LUT, MoonlyRandom, monolith_photon_dir};
pub use crate::gi_system::update_gi_light_field;

/// Minimum virtual bounce light radius in meters for the enhanced model.
pub const MIN_VIRTUAL_GI_RADIUS_METERS: f32 = 0.25;

/// Cone narrowing factor for enhanced spot GI photon rays.
pub const SPOT_GI_CONE_SCALE: f32 = 0.8;

/// Axis-aligned reflector box for real-time GI photon raycasting.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct GiReflectorBox {
    /// Minimum world-space corner of the box.
    pub min: Vec3,
    /// Maximum world-space corner of the box.
    pub max: Vec3,
    /// Diffuse surface albedo tint applied by the enhanced model.
    pub albedo: Vec3,
}

impl GiReflectorBox {
    /// Create a reflector box centered at `center` with `half_extents` and `albedo`.
    #[must_use]
    pub fn from_center_extents(center: Vec3, half_extents: Vec3, albedo: Vec3) -> Self {
        Self {
            min: center - half_extents,
            max: center + half_extents,
            albedo,
        }
    }

    /// Check whether a sphere around `origin` of radius `range` intersects this box.
    #[must_use]
    pub fn overlaps_sphere(&self, origin: Vec3, range: f32) -> bool {
        if !origin.is_finite()
            || !self.min.is_finite()
            || !self.max.is_finite()
            || !self.min.cmple(self.max).all()
            || !range.is_finite()
            || range < 0.0
        {
            return false;
        }
        let clamped = origin.clamp(self.min, self.max);
        clamped.distance_squared(origin) <= range * range
    }
}

/// Resource storing all active virtual GI bounce lights for the current frame.
#[derive(Resource, Debug, Default, Clone, Reflect)]
#[reflect(Resource)]
pub struct GiLightField {
    /// Spawned indirect virtual lights (`indirect` in `light.h`).
    pub virtual_lights: Vec<VirtualGiLight>,
}

/// Generate virtual lights with the Monolith `8N` photon selection algorithm.
pub fn gi_generate_monolith<G: GiRayGeometry + ?Sized>(
    light: &MoonlyLight,
    cfg: &GiRadiosityConfig,
    geometry: &G,
    out_virtual: &mut Vec<VirtualGiLight>,
) {
    if !valid_gi_inputs(light, cfg) || matches!(light.light_type, MoonlyLightType::Direct) {
        return;
    }
    let first = out_virtual.len();
    let Some(ray_count) = cfg.num_photons.checked_mul(8) else {
        return;
    };
    let lut = &*MONOLITH_LUT;
    let is_point = matches!(light.light_type, MoonlyLightType::Point);
    let tan_cone = if is_point {
        0.0
    } else {
        light.cone_angle.tan()
    };
    let inv_range = light.range.recip();
    let mut fallback_rng = (ray_count as usize > lut.point_dirs.len()).then(MoonlyRandom::new);

    for idx in 0..(ray_count as usize) {
        let direction = if is_point {
            if let Some(&dir) = lut.point_dirs.get(idx) {
                if dir == Vec3::ZERO {
                    continue;
                }
                dir
            } else if let Some(rng) = fallback_rng.as_mut() {
                let Some(dir) = monolith_photon_dir(light, rng) else {
                    continue;
                };
                dir
            } else {
                continue;
            }
        } else if let Some(&(random_dir, unit_offset)) = lut.spot_samples.get(idx) {
            let dir = (light.direction + random_dir * (unit_offset * tan_cone)).normalize_or_zero();
            if dir == Vec3::ZERO {
                continue;
            }
            dir
        } else if let Some(rng) = fallback_rng.as_mut() {
            let Some(dir) = monolith_photon_dir(light, rng) else {
                continue;
            };
            dir
        } else {
            continue;
        };
        let ray = GiRayQuery::new(light.position, direction, 0.0, light.range);
        let Some(hit) = geometry.nearest_hit(ray) else {
            continue;
        };
        let energy = hit.normal.dot(-direction) * (1.0 - hit.distance * inv_range);
        if !energy.is_finite() || energy < 0.0 || energy < cfg.energy_clip {
            continue;
        }
        out_virtual.push(VirtualGiLight {
            position: light.position + direction * hit.distance,
            direction: direction - hit.normal * (2.0 * direction.dot(hit.normal)),
            color: Vec3::splat(energy),
            range: 0.0,
            render_layers: light.render_layers,
        });
    }
    normalize_monolith_lights(light, cfg, first, out_virtual);
}

/// Generate enhanced diffuse GI with albedo, broadphase pruning, and optional cosine weighting.
pub fn gi_generate_enhanced(
    light: &MoonlyLight,
    cfg: &GiRadiosityConfig,
    reflectors: &[GiReflectorBox],
    out_virtual: &mut Vec<VirtualGiLight>,
    local_scratch: &mut Vec<GiReflectorBox>,
) {
    if !valid_gi_inputs(light, cfg) || !cfg.clip_distance.is_finite() {
        return;
    }
    let Some(basis) = EnhancedLightBasis::new(light, cfg.num_photons, cfg.cosine_weighted) else {
        return;
    };
    local_scratch.clear();
    local_scratch.extend(
        reflectors
            .iter()
            .filter(|r| r.overlaps_sphere(light.position, light.range))
            .copied(),
    );
    if local_scratch.is_empty() {
        return;
    }
    let inv_photons = (cfg.num_photons as f32).recip();
    for idx in 0..cfg.num_photons {
        let dir = basis.direction(idx);
        let Some((dist, normal, albedo)) = raycast_all_reflectors(
            light.position,
            dir,
            light.range,
            cfg.clip_distance,
            local_scratch,
        ) else {
            continue;
        };
        let cos_boost = if cfg.cosine_weighted {
            dir.dot(light.direction).abs().max(0.35)
        } else {
            1.0
        };
        let weight = albedo * (cos_boost * inv_photons);
        push_virtual_bounce(
            light,
            cfg,
            (light.position + dir * dist, normal, dist, weight),
            out_virtual,
        );
    }
}

fn valid_gi_inputs(light: &MoonlyLight, cfg: &GiRadiosityConfig) -> bool {
    cfg.enabled
        && cfg.num_photons > 0
        && light.flags.contains(MoonlyLightFlags::GI_ENABLED)
        && light.range.is_finite()
        && light.range > 0.0
        && light.position.is_finite()
        && light.color.is_finite()
        && cfg.reflectivity.is_finite()
        && cfg.reflectivity > 0.0
        && cfg.energy_clip.is_finite()
}

fn normalize_monolith_lights(
    light: &MoonlyLight,
    cfg: &GiRadiosityConfig,
    first: usize,
    out_virtual: &mut Vec<VirtualGiLight>,
) {
    let keep = cfg.num_photons as usize;
    let Some(candidates) = out_virtual.get_mut(first..) else {
        return;
    };
    if candidates.len() > keep {
        let _ = candidates.select_nth_unstable_by(keep, |a, b| b.color.x.total_cmp(&a.color.x));
    }
    out_virtual.truncate(first.saturating_add(keep));
    let Some(kept) = out_virtual.get_mut(first..) else {
        return;
    };
    kept.sort_unstable_by(|a, b| b.color.x.total_cmp(&a.color.x));
    let total_energy: f32 = kept.iter().map(|l| l.color.x).sum();
    if !total_energy.is_finite() || total_energy <= 0.0 {
        out_virtual.truncate(first);
        return;
    }
    let scale = cfg.reflectivity / total_energy;
    let intensity = (light.color.x + light.color.y + light.color.z) * (1.0 / 3.0);
    let mut write = first;
    for read in first..out_virtual.len() {
        let Some(&cand) = out_virtual.get(read) else {
            break;
        };
        let energy = cand.color.x * scale;
        let reflected_energy = intensity * energy;
        if let Some(range) = reference_range(reflected_energy, cfg.energy_clip) {
            if let Some(slot) = out_virtual.get_mut(write) {
                *slot = VirtualGiLight {
                    color: light.color * energy,
                    range,
                    ..cand
                };
            }
            write += 1;
        }
    }
    out_virtual.truncate(write);
}

fn push_virtual_bounce(
    light: &MoonlyLight,
    cfg: &GiRadiosityConfig,
    hit: (Vec3, Vec3, f32, Vec3),
    out_virtual: &mut Vec<VirtualGiLight>,
) {
    let (hit_pos, normal, dist, weight_albedo) = hit;
    let linear_s = (1.0 - dist / light.range).clamp(0.0, 1.0);
    let s = linear_s * linear_s * linear_s;
    let virtual_range = light.range * s * 0.5;
    if virtual_range < MIN_VIRTUAL_GI_RADIUS_METERS {
        return;
    }
    let color = light.color * weight_albedo * (s * cfg.reflectivity);
    if color.max_element() <= cfg.energy_clip {
        return;
    }
    out_virtual.push(VirtualGiLight {
        position: hit_pos + normal * cfg.normal_standoff,
        direction: normal,
        color,
        range: virtual_range,
        render_layers: light.render_layers,
    });
}

#[cfg(test)]
mod tests;

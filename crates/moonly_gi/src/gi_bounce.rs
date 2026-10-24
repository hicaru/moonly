//! Virtual GI bounce generation with Monolith and enhanced reflector models.

use bevy::prelude::*;
use moonly_core::{GiRadiosityConfig, VirtualGiLight, XRayLight, XRayLightFlags};

pub(crate) use crate::gi_intersection::raycast_all_reflectors;
use crate::gi_sampling::{XrayRandom, enhanced_photon_dir, monolith_photon_dir};
pub use crate::gi_system::update_gi_light_field;

/// Minimum virtual bounce light radius in meters for the enhanced model.
pub const MIN_VIRTUAL_GI_RADIUS_METERS: f32 = 0.25;

/// Cone narrowing factor for enhanced spot GI photon rays.
pub const SPOT_GI_CONE_SCALE: f32 = 0.8;
const MONOLITH_MIN_ENERGY: f32 = 1.0 / 255.0;
const MONOLITH_MIN_RANGE: f32 = 0.1;

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
///
/// The reflector boxes replace the original triangle collision model.
pub fn gi_generate_monolith(
    light: &XRayLight,
    cfg: &GiRadiosityConfig,
    reflectors: &[GiReflectorBox],
    out_virtual: &mut Vec<VirtualGiLight>,
) {
    if !valid_gi_inputs(light, cfg) {
        return;
    }
    let first = out_virtual.len();
    let Some(ray_count) = cfg.num_photons.checked_mul(8) else {
        return;
    };
    let mut random = XrayRandom::new();
    for _ in 0..ray_count {
        let Some(dir) = monolith_photon_dir(light, &mut random) else {
            continue;
        };
        let Some((dist, normal, _)) =
            raycast_all_reflectors(light.position, dir, light.range, 0.0, reflectors)
        else {
            continue;
        };
        let energy = normal.dot(-dir) * (1.0 - dist / light.range);
        if !energy.is_finite() || energy < 0.0 || energy < cfg.energy_clip {
            continue;
        }
        out_virtual.push(VirtualGiLight {
            position: light.position + dir * dist,
            direction: dir - normal * (2.0 * dir.dot(normal)),
            color: Vec3::splat(energy),
            range: 0.0,
        });
    }
    normalize_monolith_lights(light, cfg, first, out_virtual);
}

/// Generate enhanced diffuse GI with albedo, broadphase pruning, and optional cosine weighting.
pub fn gi_generate_enhanced(
    light: &XRayLight,
    cfg: &GiRadiosityConfig,
    reflectors: &[GiReflectorBox],
    out_virtual: &mut Vec<VirtualGiLight>,
    local_scratch: &mut Vec<GiReflectorBox>,
) {
    if !valid_gi_inputs(light, cfg) || !cfg.clip_distance.is_finite() {
        return;
    }
    local_scratch.clear();
    local_scratch.extend(
        reflectors
            .iter()
            .filter(|r| r.overlaps_sphere(light.position, light.range))
            .copied(),
    );
    for idx in 0..cfg.num_photons {
        let Some(dir) = enhanced_photon_dir(light, idx, cfg.num_photons, cfg.cosine_weighted)
        else {
            continue;
        };
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
        let weight = albedo * (cos_boost / cfg.num_photons as f32);
        push_virtual_bounce(
            light,
            cfg,
            (light.position + dir * dist, normal, dist, weight),
            out_virtual,
        );
    }
}

fn valid_gi_inputs(light: &XRayLight, cfg: &GiRadiosityConfig) -> bool {
    cfg.enabled
        && cfg.num_photons > 0
        && light.flags.contains(XRayLightFlags::GI_ENABLED)
        && light.range.is_finite()
        && light.range > 0.0
        && light.position.is_finite()
        && light.color.is_finite()
        && cfg.reflectivity.is_finite()
        && cfg.reflectivity > 0.0
        && cfg.energy_clip.is_finite()
}

fn normalize_monolith_lights(
    light: &XRayLight,
    cfg: &GiRadiosityConfig,
    first: usize,
    out_virtual: &mut Vec<VirtualGiLight>,
) {
    let Some(candidates) = out_virtual.get_mut(first..) else {
        return;
    };
    candidates.sort_unstable_by(|a, b| b.color.x.total_cmp(&a.color.x));
    out_virtual.truncate(first.saturating_add(cfg.num_photons as usize));
    let total_energy: f32 = out_virtual
        .get(first..)
        .unwrap_or_default()
        .iter()
        .map(|light| light.color.x)
        .sum();
    if !total_energy.is_finite() || total_energy <= 0.0 {
        out_virtual.truncate(first);
        return;
    }
    let scale = cfg.reflectivity / total_energy;
    let intensity = (light.color.x + light.color.y + light.color.z) / 3.0;
    let mut index = 0_usize;
    out_virtual.retain(|candidate| {
        let current = index;
        index += 1;
        let energy = intensity * candidate.color.x * scale;
        let range = (energy - MONOLITH_MIN_ENERGY) / MONOLITH_MIN_ENERGY;
        current < first || (energy >= cfg.energy_clip && range >= MONOLITH_MIN_RANGE)
    });
    let Some(candidates) = out_virtual.get_mut(first..) else {
        return;
    };
    for candidate in candidates {
        let energy = candidate.color.x * scale;
        let reflected_energy = intensity * energy;
        candidate.color = light.color * energy;
        candidate.range = (reflected_energy - MONOLITH_MIN_ENERGY) / MONOLITH_MIN_ENERGY;
    }
}

fn push_virtual_bounce(
    light: &XRayLight,
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
    });
}

#[cfg(test)]
mod tests;

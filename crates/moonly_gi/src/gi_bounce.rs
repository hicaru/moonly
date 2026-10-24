//! Real-time GI radiosity photon bounce generator ported from `light_GI.cpp`.

use bevy::prelude::*;
use moonly_core::{
    GiRadiosityConfig, LightPackage, MoonlyConfig, MoonlyPipelineTier, VirtualGiLight, XRayLight,
    XRayLightFlags, XRayLightType,
};

/// Minimum virtual bounce light radius in meters (`0.25f` in `light_GI.cpp`).
pub const MIN_VIRTUAL_GI_RADIUS_METERS: f32 = 0.25;

/// Cone narrowing factor for spot GI photon rays (`0.8f` in `light_GI.cpp`).
pub const SPOT_GI_CONE_SCALE: f32 = 0.8;

/// Golden angle increment for Fibonacci spherical photon distribution.
const GOLDEN_ANGLE_RAD: f32 = 2.399_963_1;

/// Axis-aligned reflector box for real-time GI photon raycasting.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct GiReflectorBox {
    /// Minimum world-space corner of the box.
    pub min: Vec3,
    /// Maximum world-space corner of the box.
    pub max: Vec3,
    /// Diffuse surface albedo tint applied to reflected photons.
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

/// Faithful XRay-Monolith 32-photon GI radiosity generator (`light::gi_generate`).
pub fn gi_generate_monolith(
    light: &XRayLight,
    cfg: &GiRadiosityConfig,
    reflectors: &[GiReflectorBox],
    out_virtual: &mut Vec<VirtualGiLight>,
) {
    if !cfg.enabled || !light.flags.contains(XRayLightFlags::GI_ENABLED) {
        return;
    }
    let count = cfg.num_photons.max(1);
    let inv_count = 1.0 / (count as f32);

    for idx in 0..count {
        let dir = sample_photon_dir(light, idx, count, false);
        let Some((dist, normal, albedo)) = raycast_all_reflectors(
            light.position,
            dir,
            light.range,
            cfg.clip_distance,
            reflectors,
        ) else {
            continue;
        };
        push_virtual_bounce(
            light,
            cfg,
            (
                light.position + dir * dist,
                normal,
                dist,
                albedo * inv_count,
            ),
            out_virtual,
        );
    }
}

/// Enhanced Moonly GI generator with sphere-box broadphase pruning and cosine weighting.
pub fn gi_generate_enhanced(
    light: &XRayLight,
    cfg: &GiRadiosityConfig,
    reflectors: &[GiReflectorBox],
    out_virtual: &mut Vec<VirtualGiLight>,
    local_scratch: &mut Vec<GiReflectorBox>,
) {
    if !cfg.enabled || !light.flags.contains(XRayLightFlags::GI_ENABLED) {
        return;
    }
    local_scratch.clear();
    for r in reflectors {
        if r.overlaps_sphere(light.position, light.range) {
            local_scratch.push(*r);
        }
    }
    if local_scratch.is_empty() {
        return;
    }
    let count = cfg.num_photons.max(1);
    let inv_count = 1.0 / (count as f32);

    for idx in 0..count {
        let dir = sample_photon_dir(light, idx, count, cfg.cosine_weighted);
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
        push_virtual_bounce(
            light,
            cfg,
            (
                light.position + dir * dist,
                normal,
                dist,
                albedo * (inv_count * cos_boost),
            ),
            out_virtual,
        );
    }
}

fn push_virtual_bounce(
    light: &XRayLight,
    cfg: &GiRadiosityConfig,
    hit: (Vec3, Vec3, f32, Vec3),
    out_virtual: &mut Vec<VirtualGiLight>,
) {
    let (hit_pos, normal, dist, weight_albedo) = hit;
    let linear_s = (1.0 - dist / light.range.max(0.001)).clamp(0.0, 1.0);
    let s = linear_s * linear_s * linear_s;
    let virtual_range = light.range * s * 0.5;
    if virtual_range < MIN_VIRTUAL_GI_RADIUS_METERS {
        return;
    }
    let clr = light.color * weight_albedo * (s * cfg.reflectivity);
    if clr.max_element() <= cfg.energy_clip {
        return;
    }
    out_virtual.push(VirtualGiLight {
        position: hit_pos + normal * cfg.normal_standoff,
        direction: normal,
        color: clr,
        range: virtual_range,
    });
}

fn sample_photon_dir(light: &XRayLight, idx: u32, count: u32, cosine_bias: bool) -> Vec3 {
    let t = (idx as f32 + 0.5) / (count as f32);
    let phi = (idx as f32) * GOLDEN_ANGLE_RAD;
    if matches!(
        light.light_type,
        XRayLightType::Spot | XRayLightType::Omnipart
    ) {
        let half_cone = (light.cone_angle * 0.5 * SPOT_GI_CONE_SCALE).clamp(0.05, 1.5);
        let u = if cosine_bias { t.sqrt() } else { t };
        let cos_theta = 1.0 - u * (1.0 - half_cone.cos());
        let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
        let fwd = light.direction.normalize_or_zero();
        let right = light.right.normalize_or_zero();
        let up = fwd.cross(right).normalize_or_zero();
        (right * (phi.cos() * sin_theta) + up * (phi.sin() * sin_theta) + fwd * cos_theta)
            .normalize_or_zero()
    } else {
        let z = 1.0 - 2.0 * t;
        let r = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(r * phi.cos(), z, r * phi.sin())
    }
}

fn raycast_all_reflectors(
    origin: Vec3,
    dir: Vec3,
    max_dist: f32,
    min_dist: f32,
    reflectors: &[GiReflectorBox],
) -> Option<(f32, Vec3, Vec3)> {
    let mut best_t = max_dist;
    let mut best_hit = None;
    for b in reflectors {
        if let Some((t, normal)) = intersect_ray_aabb(origin, dir, b.min, b.max)
            && t >= min_dist
            && t < best_t
        {
            best_t = t;
            best_hit = Some((t, normal, b.albedo));
        }
    }
    best_hit
}

fn intersect_ray_aabb(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<(f32, Vec3)> {
    let safe_dir = Vec3::new(
        dir.x.abs().max(1.0e-6).copysign(dir.x),
        dir.y.abs().max(1.0e-6).copysign(dir.y),
        dir.z.abs().max(1.0e-6).copysign(dir.z),
    );
    let inv = Vec3::ONE / safe_dir;
    let t1 = (min - origin) * inv;
    let t2 = (max - origin) * inv;
    let tmin_v = t1.min(t2);
    let tmax_v = t1.max(t2);
    let t_enter = tmin_v.max_element();
    let t_exit = tmax_v.min_element();
    if t_enter > t_exit || t_exit < 0.0 {
        return None;
    }
    let t = if t_enter >= 0.0 { t_enter } else { t_exit };
    let normal = if (t_enter - tmin_v.x).abs() < 1.0e-5 {
        Vec3::new(-dir.x.signum(), 0.0, 0.0)
    } else if (t_enter - tmin_v.y).abs() < 1.0e-5 {
        Vec3::new(0.0, -dir.y.signum(), 0.0)
    } else {
        Vec3::new(0.0, 0.0, -dir.z.signum())
    };
    Some((t, normal))
}

/// Bevy system that updates the `GiLightField` resource from active lights and reflector boxes.
pub fn update_gi_light_field(
    config: Res<MoonlyConfig>,
    package: Res<LightPackage>,
    reflector_query: Query<&GiReflectorBox>,
    mut field: ResMut<GiLightField>,
    mut buffers: Local<(Vec<GiReflectorBox>, Vec<GiReflectorBox>)>,
) {
    let (ref mut reflectors, ref mut scratch) = *buffers;
    reflectors.clear();
    for r in &reflector_query {
        reflectors.push(*r);
    }
    field.virtual_lights.clear();
    for light in &package.v_shadowed {
        if matches!(config.tier, MoonlyPipelineTier::MoonlyEnhanced) {
            gi_generate_enhanced(
                light,
                &config.gi,
                reflectors,
                &mut field.virtual_lights,
                scratch,
            );
        } else {
            gi_generate_monolith(light, &config.gi, reflectors, &mut field.virtual_lights);
        }
    }
}

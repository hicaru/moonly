//! Ambient occlusion (SSAO, HDAO, Activision colored AO, SSDO) and indirect bounce shading.

use bevy::prelude::*;
use moonly_core::VirtualGiLight;

/// HDAO ring reject radius in meters (`g_fHDAORejectRadius`).
pub const DEFAULT_HDAO_REJECT_RADIUS: f32 = 0.45;

/// HDAO accept radius in meters (`g_fHDAOAcceptRadius`).
pub const DEFAULT_HDAO_ACCEPT_RADIUS: f32 = 0.03;

pub use moonly_core::AmbientOcclusionSettings;

/// Evaluate Activision S2016 multi-bounce colored AO (`compute_colored_ao` in `common_functions.h`).
#[inline]
#[must_use]
pub fn compute_colored_ao_cpu(ao: f32, albedo: Vec3) -> Vec3 {
    let ao_c = ao.clamp(0.0, 1.0);
    let ch = |alb: f32| {
        let a = 2.0404 * alb - 0.3324;
        let b = -4.7951 * alb + 0.6417;
        let c = 2.7552 * alb + 0.6903;
        (((ao_c * a + b) * ao_c + c) * ao_c).max(ao_c)
    };
    Vec3::new(ch(albedo.x), ch(albedo.y), ch(albedo.z))
}

/// Accumulate reflected virtual GI lights at a world surface point (`accum_reflected`).
#[must_use]
pub fn evaluate_reflected_gi_at_point(
    world_pos: Vec3,
    world_normal: Vec3,
    virtual_lights: &[VirtualGiLight],
) -> Vec3 {
    let mut accum = Vec3::ZERO;
    for vlight in virtual_lights {
        let to_light = vlight.position - world_pos;
        let dist_sq = to_light.length_squared();
        let range_sq = vlight.range * vlight.range;
        if dist_sq >= range_sq || dist_sq < 1.0e-6 {
            continue;
        }
        let dist = dist_sq.sqrt();
        let l_dir = to_light / dist;
        let ndotl = world_normal.dot(l_dir).max(0.0);
        if ndotl <= 0.0 {
            continue;
        }
        let emit_cos = vlight.direction.dot(-l_dir).max(0.15);
        let atten = (1.0 - dist / vlight.range).clamp(0.0, 1.0);
        accum += vlight.color * (ndotl * emit_cos * atten * atten);
    }
    accum
}

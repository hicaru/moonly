//! Karis-weighted HDR10 bloom, dual-wave Gaussian weights, and procedural lens flare (`hdr10_bloom.h`).

use bevy::prelude::*;

/// Broad Gaussian sigma from `r4_rendertarget_phase_bloom.cpp` (`r_base = 3.3f`).
pub const BLOOM_GAUSS_BASE_SIGMA: f32 = 3.3;

/// Detail Gaussian sigma from `r4_rendertarget_phase_bloom.cpp` (`r_detail = 1.0f`).
pub const BLOOM_GAUSS_DETAIL_SIGMA: f32 = 1.0;

/// Compute the Karis firefly suppression weight `1.0 / (1.0 + luma(rgb))` (`hdr10_bloom_downsample.ps`).
#[must_use]
pub fn karis_weight(rgb: Vec3) -> f32 {
    let luma = rgb.dot(Vec3::new(0.2126, 0.7152, 0.0722)).max(0.0);
    1.0 / (1.0 + luma)
}

/// Average one 4-tap group and apply its Karis firefly suppression weight.
#[must_use]
pub fn karis_box4_average(c0: Vec3, c1: Vec3, c2: Vec3, c3: Vec3) -> Vec3 {
    let avg = (c0 + c1 + c2 + c3) * 0.25;
    avg * karis_weight(avg)
}

/// Combine five box groups with the normalized Karis average from Moonly-Monolith.
#[must_use]
pub fn karis_group5_average(groups: [Vec3; 5]) -> Vec3 {
    let mut weighted = Vec3::ZERO;
    let mut weight_sum = 0.0;
    for group in groups {
        let weight = karis_weight(group);
        weighted += group * weight;
        weight_sum += weight;
    }
    weighted / weight_sum.max(1.0e-8)
}

/// Evaluate the original 5-group, 13-sample Karis bloom downsample.
#[must_use]
pub fn karis_downsample_13(samples: [Vec3; 13]) -> Vec3 {
    let [a, b, c, d, e, f, g, h, i, j, k, l, m] = samples;
    let groups = [
        (a + b + d + e) * 0.25,
        (b + c + e + f) * 0.25,
        (d + e + g + h) * 0.25,
        (e + f + h + i) * 0.25,
        (j + k + l + m) * 0.25,
    ];
    karis_group5_average(groups)
}

/// Compute normalized 8-tap dual-wave Gaussian weights (`CalcGauss_wave` in `phase_bloom.cpp`).
#[must_use]
pub fn calc_gauss_dual_wave(detail_blend: f32) -> [f32; 8] {
    let k = detail_blend.clamp(0.0, 1.0);
    let mut raw = [0.0_f32; 8];
    let mut sum = 0.0_f32;
    for (idx, dst) in raw.iter_mut().enumerate() {
        let dist = idx as f32;
        let g_base = (-dist * dist / (2.0 * BLOOM_GAUSS_BASE_SIGMA * BLOOM_GAUSS_BASE_SIGMA)).exp();
        let g_det =
            (-dist * dist / (2.0 * BLOOM_GAUSS_DETAIL_SIGMA * BLOOM_GAUSS_DETAIL_SIGMA)).exp();
        let w = (1.0 - k) * g_base + k * g_det;
        *dst = w;
        sum += if idx == 0 { w } else { 2.0 * w };
    }
    let inv = 1.0 / sum.max(1.0e-5);
    for w in &mut raw {
        *w *= inv;
    }
    raw
}

/// Evaluate procedural odd-symmetry lens flare ghost and halo UV (`hdr10_lens_flare_fgen.ps`).
#[must_use]
pub fn evaluate_lens_flare_ghost(
    uv: Vec2,
    light_uv: Vec2,
    ghost_dispersal: f32,
    halo_radius: f32,
) -> Vec3 {
    let flipped = Vec2::ONE - uv;
    let to_center = Vec2::splat(0.5) - flipped;
    let mut accum = Vec3::ZERO;
    for i in 1..=4 {
        let step = (i as f32) * ghost_dispersal;
        let ghost_uv = flipped + to_center * step;
        let dist = ghost_uv.distance(light_uv);
        let weight = (1.0 - (dist * 4.5).clamp(0.0, 1.0)).powi(3);
        let chroma = Vec3::new(weight * 1.1, weight, weight * 0.9);
        accum += chroma * 0.25;
    }
    let halo_vec = to_center.normalize_or_zero() * halo_radius;
    let halo_dist = (flipped + halo_vec).distance(Vec2::splat(0.5));
    let halo_w = (1.0 - ((halo_dist - halo_radius).abs() * 8.0).clamp(0.0, 1.0)).powi(4);
    accum + Vec3::new(0.8, 0.9, 1.1) * (halo_w * 0.35)
}

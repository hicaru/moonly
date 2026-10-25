//! Karis-weighted HDR10 bloom, dual-wave Gaussian weights, and procedural lens flare (`hdr10_bloom.h`).

use bevy::prelude::*;

/// Broad Gaussian sigma from `r4_rendertarget_phase_bloom.cpp` (`r_base = 3.3f`).
pub const BLOOM_GAUSS_BASE_SIGMA: f32 = 3.3;

/// Detail Gaussian sigma from `r4_rendertarget_phase_bloom.cpp` (`r_detail = 1.0f`).
pub const BLOOM_GAUSS_DETAIL_SIGMA: f32 = 1.0;

/// Compute the Karis firefly suppression weight `1.0 / (1.0 + luma(rgb))` (`hdr10_bloom_downsample.ps`).
#[inline]
#[must_use]
pub fn karis_weight(rgb: Vec3) -> f32 {
    let luma = (0.2126 * rgb.x + 0.7152 * rgb.y + 0.0722 * rgb.z).max(0.0);
    1.0 / (1.0 + luma)
}

/// Average one 4-tap group and apply its Karis firefly suppression weight.
#[inline]
#[must_use]
pub fn karis_box4_average(c0: Vec3, c1: Vec3, c2: Vec3, c3: Vec3) -> Vec3 {
    let ax = (c0.x + c1.x + c2.x + c3.x) * 0.25;
    let ay = (c0.y + c1.y + c2.y + c3.y) * 0.25;
    let az = (c0.z + c1.z + c2.z + c3.z) * 0.25;
    let w = 1.0 / (1.0 + (0.2126 * ax + 0.7152 * ay + 0.0722 * az).max(0.0));
    Vec3::new(ax * w, ay * w, az * w)
}

/// Combine five box groups with the normalized Karis average from Moonly-Monolith.
#[inline]
#[must_use]
pub fn karis_group5_average(groups: [Vec3; 5]) -> Vec3 {
    let [g0, g1, g2, g3, g4] = groups;
    let w0 = karis_weight(g0);
    let w1 = karis_weight(g1);
    let w2 = karis_weight(g2);
    let w3 = karis_weight(g3);
    let w4 = karis_weight(g4);
    let inv = (w0 + w1 + w2 + w3 + w4).max(1.0e-8).recip();
    Vec3::new(
        (g0.x * w0 + g1.x * w1 + g2.x * w2 + g3.x * w3 + g4.x * w4) * inv,
        (g0.y * w0 + g1.y * w1 + g2.y * w2 + g3.y * w3 + g4.y * w4) * inv,
        (g0.z * w0 + g1.z * w1 + g2.z * w2 + g3.z * w3 + g4.z * w4) * inv,
    )
}

/// Evaluate the original 5-group, 13-sample Karis bloom downsample.
#[inline]
#[must_use]
pub fn karis_downsample_13(samples: [Vec3; 13]) -> Vec3 {
    let [a, b, c, d, e, f, g, h, i, j, k, l, m] = samples;
    let de_x = d.x + e.x;
    let de_y = d.y + e.y;
    let de_z = d.z + e.z;
    let ef_x = e.x + f.x;
    let ef_y = e.y + f.y;
    let ef_z = e.z + f.z;
    let groups = [
        Vec3::new(
            (a.x + b.x + de_x) * 0.25,
            (a.y + b.y + de_y) * 0.25,
            (a.z + b.z + de_z) * 0.25,
        ),
        Vec3::new(
            (b.x + c.x + ef_x) * 0.25,
            (b.y + c.y + ef_y) * 0.25,
            (b.z + c.z + ef_z) * 0.25,
        ),
        Vec3::new(
            (de_x + g.x + h.x) * 0.25,
            (de_y + g.y + h.y) * 0.25,
            (de_z + g.z + h.z) * 0.25,
        ),
        Vec3::new(
            (ef_x + h.x + i.x) * 0.25,
            (ef_y + h.y + i.y) * 0.25,
            (ef_z + h.z + i.z) * 0.25,
        ),
        Vec3::new(
            (j.x + k.x + l.x + m.x) * 0.25,
            (j.y + k.y + l.y + m.y) * 0.25,
            (j.z + k.z + l.z + m.z) * 0.25,
        ),
    ];
    karis_group5_average(groups)
}

const GAUSS_BASE_TAP: [f32; 8] = [
    1.0,
    0.955_124_4,
    0.832_223_1,
    0.661_514_64,
    0.479_688_23,
    0.317_320_8,
    0.191_495_2,
    0.105_423_42,
];

const GAUSS_DETAIL_TAP: [f32; 8] = [
    1.0,
    0.606_530_67,
    0.135_335_28,
    0.011_108_996,
    0.000_335_462_62,
    0.000_003_726_653_2,
    1.522_998e-8,
    2.289_735e-11,
];

const GHOST_MAX_DIST_SQ: f32 = (1.0 / 4.5) * (1.0 / 4.5);

/// Compute normalized 8-tap dual-wave Gaussian weights (`CalcGauss_wave` in `phase_bloom.cpp`).
#[inline]
#[must_use]
pub fn calc_gauss_dual_wave(detail_blend: f32) -> [f32; 8] {
    let k = detail_blend.clamp(0.0, 1.0);
    let inv_k = 1.0 - k;
    let w0 = inv_k * GAUSS_BASE_TAP[0] + k * GAUSS_DETAIL_TAP[0];
    let w1 = inv_k * GAUSS_BASE_TAP[1] + k * GAUSS_DETAIL_TAP[1];
    let w2 = inv_k * GAUSS_BASE_TAP[2] + k * GAUSS_DETAIL_TAP[2];
    let w3 = inv_k * GAUSS_BASE_TAP[3] + k * GAUSS_DETAIL_TAP[3];
    let w4 = inv_k * GAUSS_BASE_TAP[4] + k * GAUSS_DETAIL_TAP[4];
    let w5 = inv_k * GAUSS_BASE_TAP[5] + k * GAUSS_DETAIL_TAP[5];
    let w6 = inv_k * GAUSS_BASE_TAP[6] + k * GAUSS_DETAIL_TAP[6];
    let w7 = inv_k * GAUSS_BASE_TAP[7] + k * GAUSS_DETAIL_TAP[7];
    let sum = w0 + 2.0 * (w1 + w2 + w3 + w4 + w5 + w6 + w7);
    let inv = 1.0 / sum.max(1.0e-5);
    [
        w0 * inv,
        w1 * inv,
        w2 * inv,
        w3 * inv,
        w4 * inv,
        w5 * inv,
        w6 * inv,
        w7 * inv,
    ]
}

/// Evaluate procedural odd-symmetry lens flare ghost and halo UV (`hdr10_lens_flare_fgen.ps`).
#[inline]
#[must_use]
pub fn evaluate_lens_flare_ghost(
    uv: Vec2,
    light_uv: Vec2,
    ghost_dispersal: f32,
    halo_radius: f32,
) -> Vec3 {
    let fx = 1.0 - uv.x;
    let fy = 1.0 - uv.y;
    let tcx = 0.5 - fx;
    let tcy = 0.5 - fy;
    let mut weight_sum = 0.0_f32;
    for i in 1..=4 {
        let step = (i as f32) * ghost_dispersal;
        let dx = fx + tcx * step - light_uv.x;
        let dy = fy + tcy * step - light_uv.y;
        let d_sq = dx * dx + dy * dy;
        if d_sq < GHOST_MAX_DIST_SQ {
            let t = 1.0 - d_sq.sqrt() * 4.5;
            weight_sum += t * t * t;
        }
    }
    let flipped = Vec2::new(fx, fy);
    let to_center = Vec2::new(tcx, tcy);
    let halo_vec = to_center.normalize_or_zero() * halo_radius;
    let halo_dist = (flipped + halo_vec).distance(Vec2::splat(0.5));
    let ht = 1.0 - ((halo_dist - halo_radius).abs() * 8.0).clamp(0.0, 1.0);
    let ht2 = ht * ht;
    let halo_w = ht2 * ht2 * 0.35;
    let gw = weight_sum * 0.25;
    Vec3::new(
        1.1 * gw + 0.8 * halo_w,
        gw + 0.9 * halo_w,
        0.9 * gw + 1.1 * halo_w,
    )
}

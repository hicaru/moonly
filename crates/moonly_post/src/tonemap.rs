//! HDR10 tonemapping operators, color space matrices, ARRI LogC, and ST.2084 PQ (`hdr10.h`).

use bevy::{
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
};

/// Classic Moonly-Monolith whitepoint constant (`1.7 * 1.7 = 2.89` in `common_functions.h`).
pub const MOONLY_CLASSIC_WHITEPOINT_SQ: f32 = 2.89;

/// Tonemapping operator selection matching `hdr10.h` and `common_functions.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum TonemapOperator {
    /// Classic Moonly Reinhard whitepoint `1.7` (`tonemap` in `common_functions.h`).
    ClassicMoonly,
    /// Extended Reinhard with whitepoint `2.0` (`HDR10_Tonemap_ExtendedReinhard_Whitepoint_2`).
    ExtendedReinhardW2,
    /// Extended Reinhard with whitepoint `3.0` (`HDR10_Tonemap_ExtendedReinhard_Whitepoint_3`).
    ExtendedReinhardW3,
    /// Krzysztof Narkowicz ACES filmic curve (`HDR10_Tonemap_ACES_Narkowicz`).
    AcesNarkowicz,
    /// Stephen Hill RRT+ODT ACES fit (`HDR10_Tonemap_ACES_Hill`).
    #[default]
    AcesHill,
    /// Hajime Uchimura Gran Turismo curve (`HDR10_Tonemap_Uchimura`).
    Uchimura,
    /// `SteveM` filmic curve (`HDR10_Tonemap_SteveM_2`).
    SteveM,
    /// John Hable Uncharted 2 filmic curve (`HDR10_Tonemap_Uncharted2_Hable`).
    Uncharted2,
    /// Troy Sobotka AgX log-sigmoid curve (`HDR10_Tonemap_AgX`).
    AgX,
}

impl TonemapOperator {
    /// Return the integer shader index (`0..=8`) for this tonemapper.
    #[must_use]
    pub const fn shader_index(self) -> u32 {
        match self {
            Self::ClassicMoonly => 0,
            Self::ExtendedReinhardW2 => 1,
            Self::ExtendedReinhardW3 => 2,
            Self::AcesNarkowicz => 3,
            Self::AcesHill => 4,
            Self::Uchimura => 5,
            Self::SteveM => 6,
            Self::Uncharted2 => 7,
            Self::AgX => 8,
        }
    }
}

/// HDR10 post-processing configuration resource (`hdr10.h`).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct Hdr10PostConfig {
    /// Active tonemapping operator.
    pub operator: TonemapOperator,
    /// Exposure multiplier before tonemapping.
    pub exposure: f32,
    /// Piecewise contrast adjustment (`[-0.5, 0.5]`).
    pub contrast: f32,
    /// Saturation-aware vibrance adjustment (`[-0.5, 0.5]`).
    pub vibrance: f32,
    /// Bloom mix weight (`[0.0, 1.0]`).
    pub bloom_intensity: f32,
    /// Procedural lens flare weight (`[0.0, 1.0]`).
    pub flare_intensity: f32,
    /// Peak display luminance in nits for ST.2084 PQ encoding (`1000.0` default).
    pub max_nits: f32,
}

impl Default for Hdr10PostConfig {
    fn default() -> Self {
        Self {
            operator: TonemapOperator::AcesHill,
            exposure: 1.1,
            contrast: 0.05,
            vibrance: 0.08,
            bloom_intensity: 0.22,
            flare_intensity: 0.12,
            max_nits: 1000.0,
        }
    }
}

impl ExtractResource<RenderApp> for Hdr10PostConfig {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

/// Evaluate the selected `TonemapOperator` on a linear RGB color (`hdr10.h`).
#[inline]
#[must_use]
pub fn evaluate_tonemap_cpu(rgb: Vec3, op: TonemapOperator) -> Vec3 {
    let x = Vec3::new(rgb.x.max(0.0), rgb.y.max(0.0), rgb.z.max(0.0));
    match op {
        TonemapOperator::ClassicMoonly => reinhard_whitepoint(x, MOONLY_CLASSIC_WHITEPOINT_SQ),
        TonemapOperator::ExtendedReinhardW2 => reinhard_whitepoint(x, 4.0),
        TonemapOperator::ExtendedReinhardW3 => reinhard_whitepoint(x, 9.0),
        TonemapOperator::AcesNarkowicz => aces_narkowicz(x),
        TonemapOperator::AcesHill => aces_hill(x),
        TonemapOperator::Uchimura => uchimura_curve(x),
        TonemapOperator::SteveM => stevem_curve(x),
        TonemapOperator::Uncharted2 => uncharted2_hable(x),
        TonemapOperator::AgX => agx_curve(x),
    }
}

#[inline]
fn reinhard_whitepoint(x: Vec3, wp_sq: f32) -> Vec3 {
    let inv_wp = wp_sq.recip();
    let f = |c: f32| ((c * (1.0 + c * inv_wp)) / (c + 1.0)).clamp(0.0, 1.0);
    Vec3::new(f(x.x), f(x.y), f(x.z))
}

#[inline]
fn aces_narkowicz(x: Vec3) -> Vec3 {
    let f = |c: f32| ((c * (2.51 * c + 0.03)) / (c * (2.43 * c + 0.59) + 0.14)).clamp(0.0, 1.0);
    Vec3::new(f(x.x), f(x.y), f(x.z))
}

#[inline]
fn aces_rrt_scalar(v: f32) -> f32 {
    let a = v * (v + 0.024_578_6) - 0.000_090_537;
    let b = v * (0.983_729 * v + 0.432_951) + 0.238_081;
    a / b
}

#[inline]
fn aces_hill(x: Vec3) -> Vec3 {
    let r = aces_rrt_scalar(0.59719 * x.x + 0.35458 * x.y + 0.04823 * x.z);
    let g = aces_rrt_scalar(0.07600 * x.x + 0.90834 * x.y + 0.01566 * x.z);
    let b = aces_rrt_scalar(0.02840 * x.x + 0.13383 * x.y + 0.83777 * x.z);
    Vec3::new(
        (1.604_75 * r - 0.531_08 * g - 0.073_67 * b).clamp(0.0, 1.0),
        (-0.102_08 * r + 1.108_13 * g - 0.006_05 * b).clamp(0.0, 1.0),
        (-0.003_27 * r - 0.072_76 * g + 1.076_02 * b).clamp(0.0, 1.0),
    )
}

fn uchimura_curve(x: Vec3) -> Vec3 {
    Vec3::new(
        uchimura_scalar(x.x),
        uchimura_scalar(x.y),
        uchimura_scalar(x.z),
    )
}

fn uchimura_scalar(x: f32) -> f32 {
    let p = 1.0_f32;
    let a = 1.0_f32;
    let m = 0.22_f32;
    let l = 0.4_f32;
    let c = 1.33_f32;
    let l0 = ((p - m) * l) / a;
    let s0 = m + l0;
    let s1 = m + a * l0;
    let c2 = (a * p) / (p - s1);
    let cp = -c2 / p;
    let w0 = 1.0 - smoothstep_scalar(0.0, m, x);
    let w2 = if x >= m + l0 { 1.0 } else { 0.0 };
    let w1 = 1.0 - w0 - w2;
    let t = m * (x / m).max(0.0).powf(c);
    let s = p - (p - s1) * (cp * (x - s0)).exp();
    let l_seg = m + a * (x - m);
    (t * w0 + l_seg * w1 + s * w2).clamp(0.0, 1.0)
}

fn stevem_curve(x: Vec3) -> Vec3 {
    let a = 10.0_f32;
    let b = 0.3_f32;
    let c = 0.5_f32;
    let d = 1.5_f32;
    ((x * (a * x + Vec3::splat(b))) / (x * (a * x + Vec3::splat(c)) + Vec3::splat(d)))
        .clamp(Vec3::ZERO, Vec3::ONE)
}

fn uncharted2_partial(x: Vec3) -> Vec3 {
    let a = 0.15_f32;
    let b = 0.50_f32;
    let c = 0.10_f32;
    let d = 0.20_f32;
    let e = 0.02_f32;
    let f = 0.30_f32;
    ((x * (a * x + Vec3::splat(c * b)) + Vec3::splat(d * e))
        / (x * (a * x + Vec3::splat(b)) + Vec3::splat(d * f)))
        - Vec3::splat(e / f)
}

fn uncharted2_hable(x: Vec3) -> Vec3 {
    let curr = uncharted2_partial(x * 2.0);
    let white_scale = Vec3::ONE / uncharted2_partial(Vec3::splat(11.2));
    (curr * white_scale).clamp(Vec3::ZERO, Vec3::ONE)
}

#[inline]
fn agx_curve(x: Vec3) -> Vec3 {
    let ix = (0.842_479_06 * x.x + 0.078_433_6 * x.y + 0.079_223_75 * x.z).max(1.0e-10);
    let iy = (0.042_328_24 * x.x + 0.878_468_64 * x.y + 0.079_166_13 * x.z).max(1.0e-10);
    let iz = (0.042_375_65 * x.x + 0.078_433_6 * x.y + 0.879_143 * x.z).max(1.0e-10);
    let sx = agx_ev_sigmoid(ix);
    let sy = agx_ev_sigmoid(iy);
    let sz = agx_ev_sigmoid(iz);
    let ox = (1.196_879 * sx - 0.098_020_88 * sy - 0.099_029_75 * sz).max(0.0);
    let oy = (-0.052_896_85 * sx + 1.151_903_1 * sy - 0.098_961_18 * sz).max(0.0);
    let oz = (-0.052_971_63 * sx - 0.098_043_45 * sy + 1.151_073_6 * sz).max(0.0);
    Vec3::new(pow_gamma_22(ox), pow_gamma_22(oy), pow_gamma_22(oz))
}

#[inline]
fn agx_ev_sigmoid(c: f32) -> f32 {
    const MIN_EV: f32 = -12.473_93;
    const INV_SPAN: f32 = 1.0 / (4.026_069 - MIN_EV);
    let t = ((c.log2() - MIN_EV) * INV_SPAN).clamp(0.0, 1.0);
    let t2 = t * t;
    let t4 = t2 * t2;
    15.5 * t4 * t2 - 40.14 * t4 * t + 31.96 * t4 - 6.868 * t2 * t + 0.4298 * t2 + 0.1191 * t
        - 0.00232
}

#[inline]
fn pow_gamma_22(c: f32) -> f32 {
    if c <= 0.0 {
        0.0
    } else {
        (c.log2() * 2.2).exp2().clamp(0.0, 1.0)
    }
}

#[inline]
fn pq_channel(c: f32, scale: f32) -> f32 {
    let y = (c * scale).clamp(0.0, 1.0);
    if y <= 0.0 {
        return 0.0;
    }
    let ym1 = (y.log2() * 0.159_301_76).exp2();
    let ratio = (0.835_937_5 + 18.851_563 * ym1) / (1.0 + 18.687_5 * ym1);
    (ratio.log2() * 78.843_75).exp2()
}

fn smoothstep_scalar(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1.0e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Encode linear Rec.2020 luminance into ST.2084 Perceptual Quantizer (`HDR10_ToPQ` in `hdr10.h`).
#[inline]
#[must_use]
pub fn encode_pq_st2084(linear_rgb: Vec3, max_nits: f32) -> Vec3 {
    let scale = max_nits * 0.000_1;
    Vec3::new(
        pq_channel(linear_rgb.x, scale),
        pq_channel(linear_rgb.y, scale),
        pq_channel(linear_rgb.z, scale),
    )
}

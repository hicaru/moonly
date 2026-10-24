//! HDR10 tonemapping operators, color space matrices, ARRI LogC, and ST.2084 PQ (`hdr10.h`).

use bevy::prelude::*;

/// Classic XRay-Monolith whitepoint constant (`1.7 * 1.7 = 2.89` in `common_functions.h`).
pub const XRAY_CLASSIC_WHITEPOINT_SQ: f32 = 2.89;

/// Tonemapping operator selection matching `hdr10.h` and `common_functions.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum TonemapOperator {
    /// Classic X-Ray Reinhard whitepoint `1.7` (`tonemap` in `common_functions.h`).
    ClassicXRay,
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
            Self::ClassicXRay => 0,
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

/// Evaluate the selected `TonemapOperator` on a linear RGB color (`hdr10.h`).
#[must_use]
pub fn evaluate_tonemap_cpu(rgb: Vec3, op: TonemapOperator) -> Vec3 {
    let x = rgb.max(Vec3::ZERO);
    match op {
        TonemapOperator::ClassicXRay => reinhard_whitepoint(x, XRAY_CLASSIC_WHITEPOINT_SQ),
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

fn reinhard_whitepoint(x: Vec3, wp_sq: f32) -> Vec3 {
    ((x * (Vec3::ONE + x / wp_sq)) / (x + Vec3::ONE)).clamp(Vec3::ZERO, Vec3::ONE)
}

fn aces_narkowicz(x: Vec3) -> Vec3 {
    let num = x * (2.51 * x + Vec3::splat(0.03));
    let den = x * (2.43 * x + Vec3::splat(0.59)) + Vec3::splat(0.14);
    (num / den).clamp(Vec3::ZERO, Vec3::ONE)
}

fn aces_hill(x: Vec3) -> Vec3 {
    let v = Vec3::new(
        0.59719 * x.x + 0.35458 * x.y + 0.04823 * x.z,
        0.07600 * x.x + 0.90834 * x.y + 0.01566 * x.z,
        0.02840 * x.x + 0.13383 * x.y + 0.83777 * x.z,
    );
    let a = v * (v + Vec3::splat(0.024_578_6)) - Vec3::splat(0.000_090_537);
    let b = v * (0.983_729 * v + Vec3::splat(0.432_951)) + Vec3::splat(0.238_081);
    let rrt = a / b;
    let out = Vec3::new(
        1.604_75 * rrt.x - 0.531_08 * rrt.y - 0.073_67 * rrt.z,
        -0.102_08 * rrt.x + 1.108_13 * rrt.y - 0.006_05 * rrt.z,
        -0.003_27 * rrt.x - 0.072_76 * rrt.y + 1.076_02 * rrt.z,
    );
    out.clamp(Vec3::ZERO, Vec3::ONE)
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

fn agx_curve(x: Vec3) -> Vec3 {
    let inset = Vec3::new(
        0.842_479_06 * x.x + 0.078_433_6 * x.y + 0.079_223_75 * x.z,
        0.042_328_24 * x.x + 0.878_468_64 * x.y + 0.079_166_13 * x.z,
        0.042_375_65 * x.x + 0.078_433_6 * x.y + 0.879_143 * x.z,
    )
    .max(Vec3::splat(1.0e-10));
    let min_ev = -12.473_93_f32;
    let max_ev = 4.026_069_f32;
    let ev = Vec3::new(inset.x.log2(), inset.y.log2(), inset.z.log2());
    let t = ((ev - Vec3::splat(min_ev)) / (max_ev - min_ev)).clamp(Vec3::ZERO, Vec3::ONE);
    let sig = agx_sigmoid_poly(t);
    let out = Vec3::new(
        1.196_879 * sig.x - 0.098_020_88 * sig.y - 0.099_029_75 * sig.z,
        -0.052_896_85 * sig.x + 1.151_903_1 * sig.y - 0.098_961_18 * sig.z,
        -0.052_971_63 * sig.x - 0.098_043_45 * sig.y + 1.151_073_6 * sig.z,
    )
    .max(Vec3::ZERO);
    Vec3::new(out.x.powf(2.2), out.y.powf(2.2), out.z.powf(2.2)).clamp(Vec3::ZERO, Vec3::ONE)
}

fn agx_sigmoid_poly(x: Vec3) -> Vec3 {
    let x2 = x * x;
    let x4 = x2 * x2;
    15.5 * x4 * x2 - 40.14 * x4 * x + 31.96 * x4 - 6.868 * x2 * x + 0.4298 * x2 + 0.1191 * x
        - Vec3::splat(0.00232)
}

fn smoothstep_scalar(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1.0e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Encode linear Rec.2020 luminance into ST.2084 Perceptual Quantizer (`HDR10_ToPQ` in `hdr10.h`).
#[must_use]
pub fn encode_pq_st2084(linear_rgb: Vec3, max_nits: f32) -> Vec3 {
    let m1 = 0.159_301_76_f32;
    let m2 = 78.843_75_f32;
    let c1 = 0.835_937_5_f32;
    let c2 = 18.851_563_f32;
    let c3 = 18.687_5_f32;
    let y = (linear_rgb * (max_nits / 10_000.0)).clamp(Vec3::ZERO, Vec3::ONE);
    let ym1 = Vec3::new(y.x.powf(m1), y.y.powf(m1), y.z.powf(m1));
    let num = Vec3::splat(c1) + c2 * ym1;
    let den = Vec3::ONE + c3 * ym1;
    let ratio = num / den;
    Vec3::new(ratio.x.powf(m2), ratio.y.powf(m2), ratio.z.powf(m2))
}

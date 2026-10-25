use std::sync::LazyLock;

use bevy::prelude::*;
use moonly_core::{MoonlyLight, MoonlyLightType};

use crate::gi_bounce::SPOT_GI_CONE_SCALE;

pub(crate) const GOLDEN_ANGLE_RAD: f32 = 2.399_963_1;
pub(crate) const MONOLITH_GI_SEED: i32 = 0x1207_1980;
const MONOLITH_RANDOM_MAX: f32 = 32767.0;
const LUT_LEN: usize = 512;
const ENHANCED_LUT_LEN: usize = 64;

pub(crate) struct MonolithLut {
    pub point_dirs: [Vec3; LUT_LEN],
    pub spot_samples: [(Vec3, f32); LUT_LEN],
}

pub(crate) static MONOLITH_LUT: LazyLock<MonolithLut> = LazyLock::new(|| {
    let mut rng_point = MoonlyRandom::new();
    let point_dirs = std::array::from_fn(|_| rng_point.direction().normalize_or_zero());
    let mut rng_spot = MoonlyRandom::new();
    let spot_samples = std::array::from_fn(|_| {
        let dir = rng_spot.direction();
        let unit = rng_spot.next_f32();
        (dir, unit)
    });
    MonolithLut {
        point_dirs,
        spot_samples,
    }
});

pub(crate) static GOLDEN_SIN_COS_LUT: LazyLock<[(f32, f32); ENHANCED_LUT_LEN]> =
    LazyLock::new(|| {
        std::array::from_fn(|idx| {
            let phi = idx as f32 * GOLDEN_ANGLE_RAD;
            (phi.cos(), phi.sin())
        })
    });

#[derive(Clone, Copy)]
pub(crate) struct EnhancedLightBasis {
    light_type: MoonlyLightType,
    inv_count: f32,
    cosine_bias: bool,
    one_minus_cos_half: f32,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
}

impl EnhancedLightBasis {
    #[inline]
    pub(crate) fn new(light: &MoonlyLight, count: u32, cosine_bias: bool) -> Option<Self> {
        if matches!(light.light_type, MoonlyLightType::Direct) || count == 0 {
            return None;
        }
        let inv_count = (count as f32).recip();
        if matches!(light.light_type, MoonlyLightType::Point) {
            return Some(Self {
                light_type: light.light_type,
                inv_count,
                cosine_bias,
                one_minus_cos_half: 0.0,
                forward: Vec3::ZERO,
                right: Vec3::ZERO,
                up: Vec3::ZERO,
            });
        }
        let half_cone = (light.cone_angle * 0.5 * SPOT_GI_CONE_SCALE).clamp(0.05, 1.5);
        let forward = light.direction.normalize_or_zero();
        let right = light.right.normalize_or_zero();
        let up = forward.cross(right).normalize_or_zero();
        Some(Self {
            light_type: light.light_type,
            inv_count,
            cosine_bias,
            one_minus_cos_half: 1.0 - half_cone.cos(),
            forward,
            right,
            up,
        })
    }

    #[inline]
    pub(crate) fn direction(&self, idx: u32) -> Vec3 {
        let t = (idx as f32 + 0.5) * self.inv_count;
        let (cos_phi, sin_phi) = GOLDEN_SIN_COS_LUT
            .get(idx as usize)
            .copied()
            .unwrap_or_else(|| {
                let phi = idx as f32 * GOLDEN_ANGLE_RAD;
                (phi.cos(), phi.sin())
            });
        match self.light_type {
            MoonlyLightType::Spot | MoonlyLightType::Omnipart => {
                let u = if self.cosine_bias { t.sqrt() } else { t };
                let cos_theta = 1.0 - u * self.one_minus_cos_half;
                let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
                (self.right * (cos_phi * sin_theta)
                    + self.up * (sin_phi * sin_theta)
                    + self.forward * cos_theta)
                    .normalize_or_zero()
            }
            MoonlyLightType::Point | MoonlyLightType::Direct => {
                let z = 1.0 - 2.0 * t;
                let radius = (1.0 - z * z).max(0.0).sqrt();
                Vec3::new(radius * cos_phi, z, radius * sin_phi)
            }
        }
    }
}

pub(crate) struct MoonlyRandom {
    state: i32,
}

impl MoonlyRandom {
    pub(crate) const fn new() -> Self {
        Self {
            state: MONOLITH_GI_SEED,
        }
    }

    const fn next_u15(&mut self) -> i32 {
        self.state = self.state.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.state >> 16) & 0x7fff
    }

    fn next_f32(&mut self) -> f32 {
        self.next_u15() as f32 / MONOLITH_RANDOM_MAX
    }

    fn ranged_f32(&mut self, max: f32) -> f32 {
        self.next_f32() * max
    }

    fn direction(&mut self) -> Vec3 {
        let z = self.ranged_f32(std::f32::consts::PI).cos();
        let angle = self.ranged_f32(std::f32::consts::TAU);
        let radius = (1.0 - z * z).max(0.0).sqrt();
        Vec3::new(radius * angle.cos(), radius * angle.sin(), z)
    }
}

pub(crate) fn monolith_photon_dir(light: &MoonlyLight, random: &mut MoonlyRandom) -> Option<Vec3> {
    let random_dir = random.direction();
    let dir = match light.light_type {
        MoonlyLightType::Point => random_dir,
        MoonlyLightType::Spot | MoonlyLightType::Omnipart => {
            let offset = random.ranged_f32(light.cone_angle.tan());
            light.direction + random_dir * offset
        }
        MoonlyLightType::Direct => return None,
    };
    let dir = dir.normalize_or_zero();
    (dir != Vec3::ZERO).then_some(dir)
}

use bevy::prelude::*;
use moonly_core::{MoonlyLight, MoonlyLightType};

use crate::gi_bounce::SPOT_GI_CONE_SCALE;

pub(crate) const GOLDEN_ANGLE_RAD: f32 = 2.399_963_1;
pub(crate) const MONOLITH_GI_SEED: i32 = 0x1207_1980;
const MONOLITH_RANDOM_MAX: f32 = 32767.0;

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

pub(crate) fn enhanced_photon_dir(
    light: &MoonlyLight,
    idx: u32,
    count: u32,
    cosine_bias: bool,
) -> Option<Vec3> {
    let t = (idx as f32 + 0.5) / count as f32;
    let phi = idx as f32 * GOLDEN_ANGLE_RAD;
    let dir = match light.light_type {
        MoonlyLightType::Spot | MoonlyLightType::Omnipart => {
            spot_photon_dir(light, t, phi, cosine_bias)
        }
        MoonlyLightType::Point => {
            let z = 1.0 - 2.0 * t;
            let radius = (1.0 - z * z).max(0.0).sqrt();
            Vec3::new(radius * phi.cos(), z, radius * phi.sin())
        }
        MoonlyLightType::Direct => return None,
    };
    Some(dir)
}

fn spot_photon_dir(light: &MoonlyLight, t: f32, phi: f32, cosine_bias: bool) -> Vec3 {
    let half_cone = (light.cone_angle * 0.5 * SPOT_GI_CONE_SCALE).clamp(0.05, 1.5);
    let u = if cosine_bias { t.sqrt() } else { t };
    let cos_theta = 1.0 - u * (1.0 - half_cone.cos());
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let forward = light.direction.normalize_or_zero();
    let right = light.right.normalize_or_zero();
    let up = forward.cross(right).normalize_or_zero();
    (right * (phi.cos() * sin_theta) + up * (phi.sin() * sin_theta) + forward * cos_theta)
        .normalize_or_zero()
}

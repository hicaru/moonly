//! Volumetric sunshafts and spot light cones ported from `r4_rendertarget_accum_direct.cpp`.

use bevy::prelude::*;

/// Jitter texture tile dimension (`64.f` in `accum_direct_volumetric`).
pub const VOLUMETRIC_JITTER_TILE: f32 = 64.0;

/// Sunshaft rendering algorithm matching `ps_r_sun_shafts_mode` (`r2_R_sun.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum SunShaftsMode {
    /// Disable sunshafts (`0`).
    Off,
    /// Raymarched shadow-map volumetric shafts (`SS_VOLUMETRIC = 1`).
    Volumetric,
    /// Screen-space radial godrays (`SS_SS_OGSE / SS_SS_MANOWAR = 2`).
    ScreenSpace,
    /// Combined raymarched and screen-space shafts (`3`).
    #[default]
    Combined,
}

/// Configuration resource for volumetric sunshafts and spot shafts.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct SunShaftsConfig {
    /// Active sunshaft evaluation mode.
    pub mode: SunShaftsMode,
    /// Scattering density multiplier (`c_sunshafts_intensity`).
    pub density: f32,
    /// Number of raymarching steps per pixel (`16..=64`).
    pub raymarch_steps: u32,
    /// Henyey-Greenstein forward scattering anisotropy `g` in `[-0.9, 0.9]`.
    pub mie_anisotropy: f32,
    /// Radial decay per sample for screen-space godrays.
    pub screen_space_decay: f32,
}

impl Default for SunShaftsConfig {
    fn default() -> Self {
        Self {
            mode: SunShaftsMode::Combined,
            density: 0.35,
            raymarch_steps: 24,
            mie_anisotropy: 0.68,
            screen_space_decay: 0.96,
        }
    }
}

impl SunShaftsConfig {
    /// Compute the 64x64 jitter UV scale (`w / 64.0, h / 64.0`) from `accum_direct_volumetric`.
    #[must_use]
    pub fn jitter_uv_scale(viewport_size: Vec2) -> Vec2 {
        viewport_size / VOLUMETRIC_JITTER_TILE
    }
}

/// Evaluate Henyey-Greenstein Mie phase function for volumetric forward scattering.
#[must_use]
pub fn henyey_greenstein_phase(cos_theta: f32, g: f32) -> f32 {
    let g_clamped = g.clamp(-0.95, 0.95);
    let g2 = g_clamped * g_clamped;
    let denom = (1.0 + g2 - 2.0 * g_clamped * cos_theta)
        .max(1.0e-4)
        .powf(1.5);
    (1.0 - g2) / (4.0 * std::f32::consts::PI * denom)
}

/// Evaluate volumetric sunshaft scattering along a view ray from `camera_pos` to `world_pos`.
#[must_use]
pub fn evaluate_volumetric_sunshaft(
    camera_pos: Vec3,
    world_pos: Vec3,
    sun_dir: Vec3,
    sun_color: Vec3,
    cfg: &SunShaftsConfig,
) -> Vec3 {
    if matches!(cfg.mode, SunShaftsMode::Off) || cfg.raymarch_steps == 0 {
        return Vec3::ZERO;
    }
    let ray_vec = world_pos - camera_pos;
    let ray_len = ray_vec.length();
    if !ray_len.is_finite() || ray_len <= 1.0e-4 {
        return Vec3::ZERO;
    }
    let view_dir = ray_vec / ray_len;
    let march_len = ray_len.min(80.0);
    let cos_theta = view_dir.dot(-sun_dir.normalize_or_zero());
    let phase = henyey_greenstein_phase(cos_theta, cfg.mie_anisotropy);

    let steps = cfg.raymarch_steps.clamp(4, 64);
    let step_len = march_len / (steps as f32);
    let mut accum = 0.0_f32;
    for i in 0..steps {
        let t = (i as f32 + 0.5) * step_len;
        let p = camera_pos + view_dir * t;
        let height_atten = (-0.08 * p.y.max(0.0)).exp();
        accum += height_atten * step_len;
    }
    sun_color * (accum * phase * cfg.density * 0.05)
}

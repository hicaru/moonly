//! Night vision, thermal heatvision, and parallax scope optics (`night_vision.h`, `heatvision.ps`, `fakescope.ps`).

use bevy::prelude::*;

/// Special optic overlay modes ported from `xray-monolith` shaders.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[reflect(Resource)]
pub enum OpticsMode {
    /// Standard human vision without optic filters.
    #[default]
    Standard,
    /// Green-phosphor night vision goggles (`night_vision.h`).
    NightVisionGreen,
    /// White-phosphor gen-3 night vision goggles (`night_vision.h`).
    NightVisionWhite,
    /// Infrared thermal imaging (`heatvision.ps`).
    ThermalHeatVision,
    /// Parallax-refraction optical scope (`fakescope.ps`).
    ParallaxScope,
}

impl OpticsMode {
    /// Return the integer shader mode index (`0..=4`) for WESL uniforms.
    #[must_use]
    pub const fn shader_index(self) -> u32 {
        match self {
            Self::Standard => 0,
            Self::NightVisionGreen => 1,
            Self::NightVisionWhite => 2,
            Self::ThermalHeatVision => 3,
            Self::ParallaxScope => 4,
        }
    }
}

/// Evaluate NightVision phosphor color and tube vignette on CPU (`night_vision.h`).
#[must_use]
pub fn evaluate_night_vision_cpu(linear_rgb: Vec3, uv: Vec2, white_phosphor: bool) -> Vec3 {
    let luma = linear_rgb.dot(Vec3::new(0.2126, 0.7152, 0.0722)).max(0.0);
    let amplified = (luma * 4.5).powf(0.75).clamp(0.0, 1.5);
    let dist = (uv - Vec2::splat(0.5)).length();
    let tube_mask = (1.0 - ((dist - 0.38) / 0.14).clamp(0.0, 1.0)).powi(2);
    let scanline = 0.94 + 0.06 * (uv.y * 540.0).sin();
    let phosphor = if white_phosphor {
        Vec3::new(0.68, 0.92, 0.98)
    } else {
        Vec3::new(0.18, 0.96, 0.28)
    };
    phosphor * (amplified * tube_mask * scanline)
}

/// Evaluate smoothstep luminance thermal gradient on CPU (`heatvision.ps`).
#[must_use]
pub fn evaluate_heat_vision_cpu(linear_rgb: Vec3) -> Vec3 {
    let heat = linear_rgb
        .dot(Vec3::new(0.299, 0.587, 0.114))
        .clamp(0.0, 1.0);
    let cold = Vec3::new(0.05, 0.08, 0.35);
    let mid = Vec3::new(0.78, 0.14, 0.52);
    let hot = Vec3::new(1.0, 0.92, 0.25);
    if heat < 0.5 {
        cold.lerp(mid, heat * 2.0)
    } else {
        mid.lerp(hot, (heat - 0.5) * 2.0)
    }
}

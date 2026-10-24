//! Temporal eye adaptation and luminance reduction ported from `r4_rendertarget_phase_luminance.cpp`.

use bevy::prelude::*;

/// Default middle-gray target from `r4_rendertarget_phase_luminance.cpp`.
pub const DEFAULT_MIDDLE_GRAY: f32 = 0.28;

/// Temporal retention factor from `phase_luminance` (`0.9f`).
pub const LUMINANCE_HISTORY_KEEP: f32 = 0.9;

/// Temporal update factor from `phase_luminance` (`0.1f`).
pub const LUMINANCE_HISTORY_BLEND: f32 = 0.1;

/// Eye adaptation state resource ported from `CRenderTarget::phase_luminance`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct LuminanceAdaptationState {
    /// Current adapted scene luminance (`f_luminance_adapt`).
    pub adapted_luminance: f32,
    /// Instantaneous measured log-average scene luminance.
    pub target_luminance: f32,
    /// Middle-gray calibration constant (`MiddleGray`).
    pub middle_gray: f32,
    /// Adaptation speed multiplier (`ps_r2_tonemap_adaptation`).
    pub adaptation_speed: f32,
    /// Minimum allowed exposure multiplier (`ps_r2_tonemap_low_lum`).
    pub min_exposure: f32,
    /// Maximum allowed exposure multiplier.
    pub max_exposure: f32,
}

impl Default for LuminanceAdaptationState {
    fn default() -> Self {
        Self {
            adapted_luminance: DEFAULT_MIDDLE_GRAY,
            target_luminance: DEFAULT_MIDDLE_GRAY,
            middle_gray: DEFAULT_MIDDLE_GRAY,
            adaptation_speed: 3.0,
            min_exposure: 0.35,
            max_exposure: 3.0,
        }
    }
}

impl LuminanceAdaptationState {
    /// Step temporal luminance adaptation (`f_luminance_adapt = 0.9 * prev + 0.1 * dt * speed * target`).
    pub fn step_adaptation(&mut self, dt: f32) {
        let step_weight = (LUMINANCE_HISTORY_BLEND * dt * self.adaptation_speed).clamp(0.0, 1.0);
        let keep_weight = (1.0 - step_weight).max(LUMINANCE_HISTORY_KEEP * (1.0 - dt.min(0.5)));
        let denom = (keep_weight + step_weight).max(1.0e-5);
        self.adapted_luminance =
            (self.adapted_luminance * keep_weight + self.target_luminance * step_weight) / denom;
        self.adapted_luminance = self.adapted_luminance.clamp(0.01, 20.0);
    }

    /// Compute the automatic exposure multiplier (`MiddleGray / adapted_luminance`).
    #[must_use]
    pub fn compute_exposure_scale(&self) -> f32 {
        (self.middle_gray / self.adapted_luminance.max(0.001))
            .clamp(self.min_exposure, self.max_exposure)
    }

    /// Reduce an array of linear RGB samples to log-average luminance (`256 -> 64 -> 8 -> 1`).
    #[must_use]
    pub fn reduce_log_luminance(samples: &[Vec3]) -> f32 {
        if samples.is_empty() {
            return DEFAULT_MIDDLE_GRAY;
        }
        let luma_weights = Vec3::new(0.2126, 0.7152, 0.0722);
        let mut log_sum = 0.0_f32;
        for &rgb in samples {
            let luma = rgb.dot(luma_weights).max(1.0e-4);
            log_sum += luma.ln();
        }
        (log_sum / (samples.len() as f32)).exp()
    }
}

/// Bevy system that advances temporal eye adaptation each frame.
pub fn update_luminance_adaptation(time: Res<Time>, mut state: ResMut<LuminanceAdaptationState>) {
    state.step_adaptation(time.delta_secs());
}

use bevy::prelude::*;

pub const REFERENCE_VOLUMETRIC_SLICES: u32 = 100;
pub const REFERENCE_VOLUMETRIC_MIN_SLICES: u32 = 10;
pub const DEFAULT_VOLUMETRIC_FADE_DISTANCE_METERS: f32 = 80.0;
pub const DEFAULT_VOLUMETRIC_DENSITY_SCALE: f32 = 0.005;
const MAX_VOLUMETRIC_SLICES: u32 = 512;

/// Shared quality and distance configuration for local volumetric lights.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct VolumetricQualityConfig {
    /// Enable local volumetric accumulation.
    pub enabled: bool,
    /// Global quality multiplier.
    pub quality: f32,
    /// Baseline slice count (`VOLUMETRIC_SLICES`).
    pub slice_count: u32,
    /// Camera distance where local scattering reaches zero.
    pub fade_distance_meters: f32,
    /// Minimum slice count for low-quality lights.
    pub min_slices: u32,
    /// Explicit conversion from integrated scattering to Moonly radiance.
    pub density_scale: f32,
}

impl Default for VolumetricQualityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            quality: 1.0,
            slice_count: REFERENCE_VOLUMETRIC_SLICES,
            fade_distance_meters: DEFAULT_VOLUMETRIC_FADE_DISTANCE_METERS,
            min_slices: REFERENCE_VOLUMETRIC_MIN_SLICES,
            density_scale: DEFAULT_VOLUMETRIC_DENSITY_SCALE,
        }
    }
}

/// CPU scheduling values uploaded to the existing volumetric march.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumetricSchedule {
    /// Number of ray-march samples.
    pub slice_count: u32,
    /// Light range divided by `slice_count`.
    pub min_step: f32,
    /// Energy normalization for one march sample.
    pub intensity_normalization: f32,
    /// Camera-distance attenuation in `[0, 1]`.
    pub distance_fade: f32,
}

impl VolumetricQualityConfig {
    /// Compute slice count, step, normalization, and camera fade.
    #[must_use]
    pub fn schedule(
        &self,
        light_quality: f32,
        camera_distance: f32,
        range: f32,
    ) -> VolumetricSchedule {
        let quality =
            finite_or(self.quality, 1.0).max(0.0) * finite_or(light_quality, 1.0).max(0.0);
        let baseline = self.slice_count.clamp(1, MAX_VOLUMETRIC_SLICES);
        let requested = (baseline as f32 * quality) as u32;
        let slice_count = requested
            .max(self.min_slices.clamp(1, MAX_VOLUMETRIC_SLICES))
            .clamp(1, MAX_VOLUMETRIC_SLICES);
        let range = finite_or(range, 0.0).max(0.0);
        let normalization = (slice_count as f32).recip();
        let fade_distance = finite_or(self.fade_distance_meters, 80.0).max(0.0);
        let distance = finite_or(camera_distance, fade_distance).max(0.0);
        let distance_fade = if !self.enabled {
            0.0
        } else if fade_distance > 0.0 {
            (1.0 - distance / fade_distance).clamp(0.0, 1.0)
        } else if distance <= 0.0 {
            1.0
        } else {
            0.0
        };
        VolumetricSchedule {
            slice_count,
            min_step: range * normalization,
            intensity_normalization: normalization,
            distance_fade,
        }
    }

    /// Return a safe radiance conversion factor.
    #[must_use]
    pub const fn density_scale(&self) -> f32 {
        finite_or(self.density_scale, DEFAULT_VOLUMETRIC_DENSITY_SCALE).max(0.0)
    }
}

const fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_quality_keeps_ten_slices_and_normalizes_energy() {
        let config = VolumetricQualityConfig::default();
        for quality in [0.0, 0.001, -2.0] {
            let schedule = config.schedule(quality, 0.0, 20.0);
            assert_eq!(schedule.slice_count, 10);
            assert!((schedule.min_step - 2.0).abs() < 1.0e-5);
            assert!((schedule.intensity_normalization - 0.1).abs() < 1.0e-5);
        }
    }

    #[test]
    fn baseline_count_does_not_change_integrated_energy() {
        let first = VolumetricQualityConfig {
            slice_count: 50,
            ..VolumetricQualityConfig::default()
        };
        let second = VolumetricQualityConfig {
            slice_count: 100,
            ..first
        };
        for config in [first, second] {
            let schedule = config.schedule(1.0, 0.0, 20.0);
            let energy = schedule.slice_count as f32 * schedule.intensity_normalization;
            assert!((energy - 1.0).abs() < 1.0e-5);
        }
    }

    #[test]
    fn fade_and_invalid_inputs_stay_bounded() {
        let config = VolumetricQualityConfig::default();
        assert_eq!(config.schedule(1.0, 80.0, 20.0).distance_fade, 0.0);
        assert_eq!(config.schedule(1.0, 100.0, 20.0).distance_fade, 0.0);
        assert_eq!(config.schedule(1.0, -4.0, 20.0).distance_fade, 1.0);
        let invalid = config.schedule(f32::NAN, f32::NAN, -1.0);
        assert_eq!(invalid.min_step, 0.0);
        assert!(invalid.intensity_normalization.is_finite());
        let disabled = VolumetricQualityConfig {
            enabled: false,
            ..config
        };
        assert_eq!(disabled.schedule(1.0, 0.0, 20.0).distance_fade, 0.0);
    }
}

//! Sun cascade footprint, bias, and transition configuration.

use bevy::prelude::*;

use crate::config::{
    DEFAULT_SUN_DIAGONAL_OFFSET_METERS, DEFAULT_SUN_FAR_METERS, DEFAULT_SUN_MIDDLE_METERS,
    DEFAULT_SUN_NEAR_METERS,
};

pub const REFERENCE_SUN_CASCADE_SIZES: [f32; 3] = [20.0, 40.0, 160.0];
pub const REFERENCE_SUN_CASCADE_BIAS_FACTOR: f32 = -0.000_002_5;
pub const REFERENCE_SUN_BLEND_BAND_FRACTION: f32 = 0.1;
const MIN_SUN_CASCADE_SIZE_METERS: f32 = 0.1;
const MAX_SUN_CASCADE_SIZE_METERS: f32 = 10_000.0;

/// Light-space footprint policy for sun cascades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum SunCascadeFootprint {
    /// Fit each projection to its camera frustum slice.
    #[default]
    FrustumFit,
    /// Use fixed R4 sizes and expand them when coverage requires it.
    ReferenceFixed,
}

/// Sun cascade configuration matching `r2_R_sun.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct SunCascadeConfig {
    /// Near cascade split distance in meters (`SE_SUN_NEAR`).
    pub near_split: f32,
    /// Middle cascade split distance in meters (`SE_SUN_MIDDLE`).
    pub middle_split: f32,
    /// Far cascade split distance in meters (`SE_SUN_FAR`).
    pub far_split: f32,
    /// Sun extrusion offset along the light direction (`c_sun_long_diagonal_offset`).
    pub diagonal_offset: f32,
    /// Legacy depth bias for the `FrustumFit` policy.
    pub depth_bias: f32,
    /// Snap cascade projection to texel grid to prevent shimmer.
    pub stabilize_texels: bool,
    /// Enable 4x4 MinMax shadow pyramid for the near cascade.
    pub use_minmax_near: bool,
    /// Fixed light-space widths for the three R4 cascades.
    pub ortho_sizes: [f32; 3],
    /// Depth bias multiplier for the `ReferenceFixed` policy.
    pub bias_factor: f32,
    /// Transition width relative to the current cascade far split.
    pub blend_band_fraction: f32,
    /// Light-space footprint policy.
    pub footprint: SunCascadeFootprint,
}

impl Default for SunCascadeConfig {
    fn default() -> Self {
        Self {
            near_split: DEFAULT_SUN_NEAR_METERS,
            middle_split: DEFAULT_SUN_MIDDLE_METERS,
            far_split: DEFAULT_SUN_FAR_METERS,
            diagonal_offset: DEFAULT_SUN_DIAGONAL_OFFSET_METERS,
            depth_bias: -0.00005,
            stabilize_texels: true,
            use_minmax_near: true,
            ortho_sizes: REFERENCE_SUN_CASCADE_SIZES,
            bias_factor: REFERENCE_SUN_CASCADE_BIAS_FACTOR,
            blend_band_fraction: REFERENCE_SUN_BLEND_BAND_FRACTION,
            footprint: SunCascadeFootprint::default(),
        }
    }
}

impl SunCascadeConfig {
    /// Create the deterministic R4 comparison preset.
    #[must_use]
    pub fn reference() -> Self {
        Self {
            near_split: 20.0,
            middle_split: 40.0,
            far_split: 160.0,
            footprint: SunCascadeFootprint::ReferenceFixed,
            ..Self::default()
        }
    }

    /// Return one valid fixed footprint size.
    #[must_use]
    pub fn ortho_size(&self, index: usize) -> f32 {
        let fallback = REFERENCE_SUN_CASCADE_SIZES
            .get(index)
            .copied()
            .or_else(|| REFERENCE_SUN_CASCADE_SIZES.last().copied())
            .unwrap_or(160.0);
        self.ortho_sizes
            .get(index)
            .copied()
            .filter(|size| size.is_finite())
            .unwrap_or(fallback)
            .clamp(MIN_SUN_CASCADE_SIZE_METERS, MAX_SUN_CASCADE_SIZE_METERS)
    }

    /// Return the transition width for one cascade.
    #[must_use]
    pub fn blend_band(&self, split_far: f32) -> f32 {
        let fraction = if self.blend_band_fraction.is_finite() {
            self.blend_band_fraction
        } else {
            REFERENCE_SUN_BLEND_BAND_FRACTION
        }
        .clamp(0.0, 0.5);
        split_far.max(0.0) * fraction
    }

    /// Return the depth bias for the active footprint policy.
    #[must_use]
    pub fn cascade_depth_bias(&self, footprint_size: f32) -> f32 {
        let factor = if self.bias_factor.is_finite() {
            self.bias_factor
        } else {
            REFERENCE_SUN_CASCADE_BIAS_FACTOR
        }
        .clamp(-1.0, 1.0);
        match self.footprint {
            SunCascadeFootprint::FrustumFit => self.depth_bias,
            SunCascadeFootprint::ReferenceFixed => footprint_size * factor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_preset_has_original_sizes_splits_and_bias() {
        let config = SunCascadeConfig::reference();
        assert!(
            config
                .ortho_sizes
                .iter()
                .eq(REFERENCE_SUN_CASCADE_SIZES.iter())
        );
        assert!(config.near_split < config.middle_split);
        assert!(config.middle_split < config.far_split);
        let bias = config.cascade_depth_bias(config.ortho_size(1));
        assert!((bias - 40.0 * REFERENCE_SUN_CASCADE_BIAS_FACTOR).abs() < 1.0e-8);
    }

    #[test]
    fn invalid_sizes_and_blend_fractions_clamp_gracefully() {
        let mut config = SunCascadeConfig::reference();
        if let Some(size) = config.ortho_sizes.get_mut(0) {
            *size = -4.0;
        }
        config.blend_band_fraction = 2.0;
        assert!((config.ortho_size(0) - MIN_SUN_CASCADE_SIZE_METERS).abs() < f32::EPSILON);
        assert!((config.blend_band(40.0) - 20.0).abs() < f32::EPSILON);
    }
}

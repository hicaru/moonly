use bevy::prelude::*;

const MAX_RAIN_DISTANCE_METERS: f32 = 10_000.0;

/// Wet-surface shadow distances and resolution from `r4_R_rain.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct RainWetSurfaceConfig {
    /// Start of the wet-surface fade in meters.
    pub near_meters: f32,
    /// End of the wet-surface fade and shadow frustum in meters.
    pub far_meters: f32,
    /// Requested rain shadow resolution in texels.
    pub smap_resolution: u32,
}

impl Default for RainWetSurfaceConfig {
    fn default() -> Self {
        Self {
            near_meters: 10.0,
            far_meters: 30.0,
            smap_resolution: 256,
        }
    }
}

impl RainWetSurfaceConfig {
    /// Return a safe near fade distance.
    #[must_use]
    pub const fn near_meters(&self) -> f32 {
        if self.near_meters.is_finite() {
            self.near_meters.clamp(0.0, MAX_RAIN_DISTANCE_METERS)
        } else {
            10.0
        }
    }

    /// Return a safe far distance beyond the near fade.
    #[must_use]
    pub const fn far_meters(&self) -> f32 {
        if self.far_meters.is_finite() {
            self.far_meters
                .clamp(self.near_meters(), MAX_RAIN_DISTANCE_METERS)
        } else {
            30.0_f32.max(self.near_meters())
        }
    }

    /// Return the effective resolution after both configured limits.
    #[must_use]
    pub fn effective_smap_size(&self, pool_size: u32) -> u32 {
        pool_size.max(1).min(self.smap_resolution.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_defaults_and_resolution_clamps_match() {
        let config = RainWetSurfaceConfig::default();
        assert_eq!(config.near_meters(), 10.0);
        assert_eq!(config.far_meters(), 30.0);
        assert_eq!(config.effective_smap_size(2048), 256);
        assert_eq!(config.effective_smap_size(128), 128);
        assert_eq!(config.effective_smap_size(0), 1);
    }
}

//! Volumetric sunshafts, spot cones, and Mie scattering for Moonly.

pub mod sunshafts;

use bevy::{asset::embedded_asset, prelude::*};

pub use crate::sunshafts::{
    SunShaftsConfig, SunShaftsMode, VOLUMETRIC_JITTER_TILE, evaluate_volumetric_sunshaft,
    henyey_greenstein_phase,
};

/// Raw WESL source for `volumetrics.wesl`.
pub const VOLUMETRICS_WESL_SOURCE: &str = include_str!("volumetrics.wesl");

/// Plugin that registers volumetric sunshafts and spot light scattering settings.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoonlyVolumetricsPlugin;

impl Plugin for MoonlyVolumetricsPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<AssetPlugin>() {
            embedded_asset!(app, "volumetrics.wesl");
        }
        app.register_type::<SunShaftsMode>()
            .register_type::<SunShaftsConfig>()
            .init_resource::<SunShaftsConfig>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_mie_scattering_is_stronger_toward_sun() {
        let forward = henyey_greenstein_phase(0.95, 0.7);
        let backward = henyey_greenstein_phase(-0.95, 0.7);
        assert!(forward > backward * 10.0);
    }

    #[test]
    fn volumetric_sunshaft_produces_positive_radiance() {
        let cfg = SunShaftsConfig::default();
        let rad = evaluate_volumetric_sunshaft(
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, 2.0, -25.0),
            Vec3::new(0.0, -0.2, 1.0).normalize(),
            Vec3::new(1.5, 1.2, 0.9),
            &cfg,
        );
        assert!(rad.length() > 0.0);
    }
}

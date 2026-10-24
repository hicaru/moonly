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

    #[test]
    fn long_ray_uses_unit_direction_and_capped_march_distance() {
        let cfg = SunShaftsConfig::default();
        let camera = Vec3::ZERO;
        let direction = Vec3::new(1.0, 0.0, -1.0).normalize();
        let sun_dir = Vec3::new(0.2, -0.3, 1.0).normalize();
        let color = Vec3::new(1.5, 1.2, 0.9);
        let long =
            evaluate_volumetric_sunshaft(camera, camera + direction * 160.0, sun_dir, color, &cfg);
        let capped =
            evaluate_volumetric_sunshaft(camera, camera + direction * 80.0, sun_dir, color, &cfg);
        assert!((long - capped).length() < 1.0e-5);
    }

    #[test]
    fn short_ray_marches_its_complete_length() {
        let cfg = SunShaftsConfig {
            raymarch_steps: 4,
            density: 1.0,
            mie_anisotropy: 0.0,
            ..SunShaftsConfig::default()
        };
        let color = Vec3::ONE;
        let actual = evaluate_volumetric_sunshaft(
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -8.0),
            Vec3::Z,
            color,
            &cfg,
        );
        let expected = 8.0 * henyey_greenstein_phase(1.0, 0.0) * 0.05;
        assert!((actual.x - expected).abs() < 1.0e-5);
    }

    #[test]
    fn zero_and_non_finite_rays_return_zero() {
        let cfg = SunShaftsConfig::default();
        for world_pos in [Vec3::ZERO, Vec3::NAN, Vec3::INFINITY] {
            let actual =
                evaluate_volumetric_sunshaft(Vec3::ZERO, world_pos, Vec3::Z, Vec3::ONE, &cfg);
            assert_eq!(actual, Vec3::ZERO);
        }
    }

    #[test]
    fn wesl_normalizes_by_full_length_and_supports_sixty_four_steps() {
        assert!(VOLUMETRICS_WESL_SOURCE.contains("let view_dir = ray_vec / ray_len;"));
        assert!(VOLUMETRICS_WESL_SOURCE.contains("let max_dist = min(ray_len, 80.0);"));
        assert!(VOLUMETRICS_WESL_SOURCE.contains("clamp(params.w, 4.0, 64.0)"));
    }
}

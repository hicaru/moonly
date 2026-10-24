//! Core configuration, light packages, hemisphere tracking, and shader bus for Moonly.

pub mod config;
pub mod hemisphere;
pub mod lights;
pub mod shader_bus;

use bevy::{asset::embedded_asset, prelude::*};

pub use crate::{
    config::{
        AmbientOcclusionMode, DEFAULT_DLIGHTS_CLIP, DEFAULT_GI_CLIP_METERS, DEFAULT_GI_PHOTONS,
        DEFAULT_GI_REFLECTIVITY, DEFAULT_SMAP_POOL_SIZE, DEFAULT_SUN_DIAGONAL_OFFSET_METERS,
        DEFAULT_SUN_FAR_METERS, DEFAULT_SUN_MIDDLE_METERS, DEFAULT_SUN_NEAR_METERS,
        GiRadiosityConfig, MoonlyConfig, MoonlyPipelineTier, ShadowFilterMode, SunCascadeConfig,
    },
    hemisphere::{
        HEMI_CUBE_DIRS, HEMI_FALL_SPEED, HEMI_RISE_SPEED, HEMI_SAMPLE_COUNT, HEMI_SAMPLE_DIRS,
        HemisphereTracker, WeatherEnvironment, update_hemisphere_trackers,
    },
    lights::{
        LightPackage, VirtualGiLight, VolumetricLightSettings, XRayLight, XRayLightFlags,
        XRayLightType, collect_light_packages,
    },
    shader_bus::{SHADER_BUS_SLOT_COUNT, ShaderBus},
};

///Raw WESL source for `core_common.wesl` (`common_functions.h`).
pub const CORE_COMMON_WESL_SOURCE: &str = include_str!("core_common.wesl");

/// Ordered execution phases for Moonly lighting, shadow, GI, and material sync in `PostUpdate`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoonlySystemSet {
    /// Gather active `XRayLight` entities into `LightPackage` and update hemisphere trackers.
    CollectLights,
    /// Compute sun shadow cascades, pack the SMAP atlas, and trace 32-photon GI bounces.
    UpdateShadowsAndGi,
    /// Upload lighting, shadow, GI, and HDR10 uniforms into `MoonlyLitMaterial` assets.
    SyncMaterials,
}

/// Core Moonly plugin that registers configuration, lights, hemisphere tracking, and shader bus.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoonlyCorePlugin;

impl Plugin for MoonlyCorePlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<AssetPlugin>() {
            embedded_asset!(app, "core_common.wesl");
        }
        app.configure_sets(
            PostUpdate,
            (
                MoonlySystemSet::CollectLights,
                MoonlySystemSet::UpdateShadowsAndGi,
                MoonlySystemSet::SyncMaterials,
            )
                .chain(),
        )
        .register_type::<MoonlyConfig>()
        .register_type::<MoonlyPipelineTier>()
        .register_type::<ShadowFilterMode>()
        .register_type::<AmbientOcclusionMode>()
        .register_type::<SunCascadeConfig>()
        .register_type::<GiRadiosityConfig>()
        .register_type::<XRayLight>()
        .register_type::<XRayLightType>()
        .register_type::<XRayLightFlags>()
        .register_type::<VolumetricLightSettings>()
        .register_type::<VirtualGiLight>()
        .register_type::<LightPackage>()
        .register_type::<WeatherEnvironment>()
        .register_type::<HemisphereTracker>()
        .register_type::<ShaderBus>()
        .init_resource::<MoonlyConfig>()
        .init_resource::<LightPackage>()
        .init_resource::<WeatherEnvironment>()
        .init_resource::<ShaderBus>()
        .add_systems(
            PostUpdate,
            (collect_light_packages, update_hemisphere_trackers)
                .chain()
                .in_set(MoonlySystemSet::CollectLights),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_package_splits_shadowed_point_into_six_omniparts() {
        let mut pkg = LightPackage::default();
        let point = XRayLight {
            light_type: XRayLightType::Point,
            flags: XRayLightFlags::SHADOW,
            ..XRayLight::default()
        };
        pkg.push(point);
        assert_eq!(pkg.v_shadowed.len(), 6);
        assert!(pkg.v_point.is_empty());
    }

    #[test]
    fn hemisphere_smoothing_uses_asymmetric_rise_and_fall_rates() {
        let risen = HemisphereTracker::smooth_value(0.2, 1.0, 0.1);
        let fallen = HemisphereTracker::smooth_value(1.0, 0.2, 0.1);
        assert!((risen - 0.52).abs() < 1.0e-5);
        assert!((fallen - 0.96).abs() < 1.0e-5);
    }

    #[test]
    fn shader_bus_reads_and_writes_one_based_slots() {
        let mut bus = ShaderBus::default();
        assert!(bus.set_slot(1, Vec4::new(1.0, 2.0, 3.0, 4.0)));
        assert!(!bus.set_slot(0, Vec4::ONE));
        assert!(!bus.set_slot(9, Vec4::ONE));
        assert_eq!(bus.get_slot(1), Some(Vec4::new(1.0, 2.0, 3.0, 4.0)));
    }
}

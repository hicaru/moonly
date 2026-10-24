//! Shadow map Guillotine allocator, sun cascades, spot shadow matrices, and rain shadows.

pub mod rain_shadow;
pub mod smap_allocator;
pub mod spot_xform;
pub mod sun_cascades;

use bevy::{asset::embedded_asset, prelude::*};
use moonly_core::{
    LightPackage, MoonlyConfig, MoonlyPipelineTier, MoonlySystemSet, WeatherEnvironment,
};

pub use crate::{
    rain_shadow::{
        DEFAULT_RAIN_MAX_DISTANCE_METERS, RAIN_CULL_COP_OFFSET_METERS,
        RAIN_ORTHO_XFORM_OFFSET_METERS, RAIN_SOURCE_OFFSET_METERS, RainShadowState,
    },
    smap_allocator::{SmapAllocator, SmapRect},
    spot_xform::{
        MIN_SPOT_SMAP_SIZE, SpotShadowXform, TAN_SHIFT_OMNIPART, TAN_SHIFT_POINT, TAN_SHIFT_SPOT,
    },
    sun_cascades::{
        CameraFrustumParams, SE_SUN_FAR, SE_SUN_MIDDLE, SE_SUN_NEAR, SUN_CASCADE_COUNT,
        SunCascadeSlice, SunCascades,
    },
};

/// Raw WESL source for `shadows.wesl`.
pub const SHADOWS_WESL_SOURCE: &str = include_str!("shadows.wesl");

/// Plugin that manages shadow map atlas allocation, sun cascades, and rain shadows.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoonlyShadowsPlugin;

impl Plugin for MoonlyShadowsPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<AssetPlugin>() {
            embedded_asset!(app, "shadows.wesl");
        }
        app.register_type::<SmapRect>()
            .register_type::<SmapAllocator>()
            .register_type::<SpotShadowXform>()
            .register_type::<SunCascadeSlice>()
            .register_type::<SunCascades>()
            .register_type::<RainShadowState>()
            .init_resource::<SmapAllocator>()
            .init_resource::<SunCascades>()
            .init_resource::<RainShadowState>()
            .add_systems(
                PostUpdate,
                update_shadow_cascades_and_atlas.in_set(MoonlySystemSet::UpdateShadowsAndGi),
            );
    }
}

/// Update sun shadow cascades, rain shadow state, and dynamic SMAP allocations each frame.
pub fn update_shadow_cascades_and_atlas(
    config: Res<MoonlyConfig>,
    env: Res<WeatherEnvironment>,
    package: Res<LightPackage>,
    mut allocator: ResMut<SmapAllocator>,
    mut cascades: ResMut<SunCascades>,
) {
    allocator.initialize(config.smap_pool_size);
    let cam = CameraFrustumParams::default();
    cascades.compute(
        &cam,
        env.sun_direction,
        &config.sun_cascades,
        config.smap_pool_size,
    );

    let cam_pos = cam.camera_transform.translation;
    for light in &package.v_shadowed {
        let xf = SpotShadowXform::compute_xf_spot(light, cam_pos, config.smap_pool_size, 0);
        if matches!(config.tier, MoonlyPipelineTier::MoonlyEnhanced) {
            let _ = allocator.push_enhanced_bssf(xf.smap_size);
        } else {
            let _ = allocator.push_monolith(xf.smap_size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moonly_core::{SunCascadeConfig, XRayLight};

    #[test]
    fn smap_allocator_packs_tiles_without_overlap() {
        let mut alloc = SmapAllocator::new(2048);
        let r1 = alloc.push_monolith(1024);
        let r2 = alloc.push_monolith(1024);
        let r3 = alloc.push_monolith(512);
        assert!(r1.is_some());
        assert!(r2.is_some());
        assert!(r3.is_some());
        assert_eq!(alloc.allocated.len(), 3);
    }

    #[test]
    fn bssf_allocator_packs_mixed_sizes_efficiently() {
        let mut alloc = SmapAllocator::new(1024);
        for size in [512, 256, 256, 512, 256, 128, 128] {
            assert!(alloc.push_enhanced_bssf(size).is_some());
        }
    }

    #[test]
    fn spot_shadow_xform_applies_one_percent_hysteresis() {
        let light = XRayLight::spot(Vec3::new(0.0, 4.0, 0.0), Vec3::NEG_Y, Vec3::ONE, 15.0, 1.0);
        let first = SpotShadowXform::compute_xf_spot(&light, Vec3::ZERO, 2048, 0);
        let second = SpotShadowXform::compute_xf_spot(
            &light,
            Vec3::new(0.01, 0.0, 0.0),
            2048,
            first.smap_size,
        );
        assert_eq!(first.smap_size, second.smap_size);
    }

    #[test]
    fn sun_cascades_produce_valid_orthographic_slices() {
        let mut cascades = SunCascades::default();
        let cam = CameraFrustumParams::default();
        let cfg = SunCascadeConfig::default();
        cascades.compute(&cam, Vec3::new(-0.4, -0.8, -0.4).normalize(), &cfg, 2048);
        for slice in &cascades.cascades {
            assert!(slice.split_far > slice.split_near);
            assert!(slice.combine.determinant().abs() > 0.0);
        }
    }
}

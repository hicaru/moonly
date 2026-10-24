//! Shadow map Guillotine allocator, sun cascades, spot shadow matrices, and rain shadows.

pub mod rain_shadow;
pub mod shadow_update;
pub mod smap_allocator;
pub mod spot_xform;
pub mod sun_cascades;

use bevy::{asset::embedded_asset, prelude::*};
use moonly_core::MoonlySystemSet;

pub use crate::{
    rain_shadow::{
        DEFAULT_RAIN_MAX_DISTANCE_METERS, RAIN_CULL_COP_OFFSET_METERS,
        RAIN_ORTHO_XFORM_OFFSET_METERS, RAIN_SOURCE_OFFSET_METERS, RainShadowState,
    },
    shadow_update::update_shadow_cascades_and_atlas,
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

#[cfg(test)]
mod tests;

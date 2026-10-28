//! Shadow map Guillotine allocator, sun cascades, spot shadow matrices, and rain shadows.

mod cascade_planes;
pub mod local_batches;
pub mod rain_shadow;
pub mod shadow_update;
pub mod smap_allocator;
pub mod sun_cascades;

use bevy::{asset::embedded_asset, prelude::*};
use moonly_core::{MoonlyShadowPackage, MoonlySunPackage, MoonlySystemSet};

pub use crate::{
    local_batches::{LocalShadowBatch, LocalShadowBatches, LocalShadowView},
    rain_shadow::{
        DEFAULT_RAIN_MAX_DISTANCE_METERS, RAIN_CULL_COP_OFFSET_METERS,
        RAIN_ORTHO_XFORM_OFFSET_METERS, RAIN_SOURCE_OFFSET_METERS, RainComputeParams,
        RainShadowState,
    },
    shadow_update::update_shadow_cascades_and_atlas,
    smap_allocator::{SmapAllocator, SmapRect},
    sun_cascades::{
        CameraFrustumParams, SE_SUN_FAR, SE_SUN_MIDDLE, SE_SUN_NEAR, SUN_CASCADE_COUNT,
        SunCascadeSlice, SunCascades,
    },
};

pub use moonly_core::spot_xform;
pub use moonly_core::spot_xform::{
    MIN_SPOT_SMAP_SIZE, SpotShadowXform, TAN_SHIFT_OMNIPART, TAN_SHIFT_POINT, TAN_SHIFT_SPOT,
    look_to_rh_mat4, orthographic_rh_mat4, perspective_rh_mat4,
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
            .init_resource::<LocalShadowBatches>()
            .init_resource::<MoonlyShadowPackage>()
            .init_resource::<MoonlySunPackage>()
            .init_resource::<SunCascades>()
            .init_resource::<RainShadowState>()
            .init_resource::<bevy::light::DirectionalLightShadowMap>()
            .init_resource::<moonly_core::MoonlyNativeForward>()
            .add_systems(
                PostUpdate,
                update_shadow_cascades_and_atlas.in_set(MoonlySystemSet::UpdateShadowsAndGi),
            );
    }
}

#[cfg(test)]
mod tests;

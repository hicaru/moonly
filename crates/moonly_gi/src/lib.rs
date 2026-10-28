//! Real-time GI photon radiosity bounce and Activision colored AO for Moonly.

pub mod ambient_occlusion;
pub mod gi_bounce;
mod gi_bvh;
mod gi_cache;
mod gi_geometry;
mod gi_intersection;
mod gi_range;
mod gi_ray;
mod gi_sampling;
mod gi_system;
mod glow;
mod light_tracking;

use bevy::{
    asset::{AssetEvent, embedded_asset},
    prelude::*,
};
use moonly_core::{MoonlySystemSet, VirtualGiLightPackage};

use crate::gi_geometry::collect_static_gi_geometry;

pub use crate::{
    ambient_occlusion::{
        AmbientOcclusionSettings, DEFAULT_HDAO_ACCEPT_RADIUS, DEFAULT_HDAO_REJECT_RADIUS,
        compute_colored_ao_cpu, evaluate_reflected_gi_at_point,
    },
    gi_bounce::{
        GiLightField, GiReflectorBox, MIN_VIRTUAL_GI_RADIUS_METERS, SPOT_GI_CONE_SCALE,
        gi_generate_enhanced, gi_generate_monolith, update_gi_light_field,
    },
    gi_bvh::{GiTriangle, GiTriangleBvh},
    gi_cache::GiLightCache,
    gi_geometry::{MoonlyGiCaster, StaticGiGeometry},
    gi_ray::{GiRayGeometry, GiRayHit, GiRayQuery},
    glow::{MoonlyGlow, MoonlyGlowCache},
};

/// Reference shader source. Runtime virtual lights use the Moonly render accumulation passes.
pub const GI_WESL_SOURCE: &str = include_str!("gi.wesl");

/// Plugin that registers GI radiosity bounce generation and ambient occlusion settings.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoonlyGiPlugin;

impl Plugin for MoonlyGiPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<AssetPlugin>() {
            embedded_asset!(app, "gi.wesl");
        }
        app.add_message::<AssetEvent<Mesh>>();
        app.register_type::<GiReflectorBox>()
            .register_type::<MoonlyGiCaster>()
            .register_type::<MoonlyGlow>()
            .register_type::<GiLightField>()
            .register_type::<AmbientOcclusionSettings>()
            .init_resource::<moonly_core::MoonlyNativeForward>()
            .init_resource::<GiLightField>()
            .init_resource::<VirtualGiLightPackage>()
            .init_resource::<GiLightCache>()
            .init_resource::<MoonlyGlowCache>()
            .init_resource::<StaticGiGeometry>()
            .init_resource::<AmbientOcclusionSettings>()
            .add_systems(
                PostUpdate,
                glow::sync_mesh_emitters.before(MoonlySystemSet::CollectLights),
            )
            .add_systems(
                PostUpdate,
                (
                    collect_static_gi_geometry,
                    light_tracking::update_light_tracking,
                    update_gi_light_field,
                )
                    .chain()
                    .in_set(MoonlySystemSet::UpdateShadowsAndGi),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moonly_core::{GiRadiosityConfig, MoonlyLight};

    #[test]
    fn gi_generate_spawns_virtual_bounce_lights_on_reflector() {
        let light = MoonlyLight::spot(
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::NEG_Y,
            Vec3::new(4.0, 3.0, 2.0),
            12.0,
            1.4,
        );
        let floor = GiReflectorBox::from_center_extents(
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(10.0, 0.5, 10.0),
            Vec3::new(0.8, 0.4, 0.2),
        );
        let cfg = GiRadiosityConfig::default();
        let mut out_mono = Vec::new();
        gi_generate_monolith(&light, &cfg, &[floor], &mut out_mono);
        assert_ne!(out_mono.len(), 0);

        let mut out_enh = Vec::new();
        let mut scratch = Vec::new();
        gi_generate_enhanced(&light, &cfg, &[floor], &mut out_enh, &mut scratch);
        assert_ne!(out_enh.len(), 0);

        let lit = evaluate_reflected_gi_at_point(Vec3::new(0.0, 0.5, 0.0), Vec3::NEG_Y, &out_enh);
        assert!(lit.length() > 0.0);
    }

    #[test]
    fn activision_colored_ao_is_never_darker_than_scalar_ao() {
        let ao = 0.5;
        let colored = compute_colored_ao_cpu(ao, Vec3::new(0.9, 0.2, 0.1));
        assert!(colored.x >= ao);
        assert!(colored.y >= ao);
        assert!(colored.z >= ao);
    }
}

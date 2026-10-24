//! HDR10 tonemapping, eye adaptation, Karis bloom, lens flare, and optic modes for Moonly.

pub mod bloom_flare;
pub mod luminance;
pub mod material;
pub mod optics;
pub mod tonemap;

use bevy::{asset::embedded_asset, pbr::MaterialPlugin, prelude::*};
use moonly_core::MoonlySystemSet;

pub use crate::{
    bloom_flare::{
        BLOOM_GAUSS_BASE_SIGMA, BLOOM_GAUSS_DETAIL_SIGMA, calc_gauss_dual_wave,
        evaluate_lens_flare_ghost, karis_box4_average, karis_weight,
    },
    luminance::{
        DEFAULT_MIDDLE_GRAY, LUMINANCE_HISTORY_BLEND, LUMINANCE_HISTORY_KEEP,
        LuminanceAdaptationState, update_luminance_adaptation,
    },
    material::{
        CoreLightingRes, MoonlyLitMaterial, MoonlyLitUniform, PostLightingRes,
        sync_moonly_lit_materials,
    },
    optics::{OpticsMode, evaluate_heat_vision_cpu, evaluate_night_vision_cpu},
    tonemap::{
        Hdr10PostConfig, TonemapOperator, XRAY_CLASSIC_WHITEPOINT_SQ, encode_pq_st2084,
        evaluate_tonemap_cpu,
    },
};

/// Raw WESL source for `tonemap.wesl`.
pub const TONEMAP_WESL_SOURCE: &str = include_str!("tonemap.wesl");

/// Raw WESL source for `bloom_optics.wesl`.
pub const BLOOM_OPTICS_WESL_SOURCE: &str = include_str!("bloom_optics.wesl");

/// Raw WESL source for `moonly_lit.wesl`.
pub const MOONLY_LIT_WESL_SOURCE: &str = include_str!("moonly_lit.wesl");

/// Plugin that registers HDR10 tonemapping, luminance adaptation, bloom, optics, and `MoonlyLitMaterial`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoonlyPostPlugin;

impl Plugin for MoonlyPostPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<AssetPlugin>() {
            embedded_asset!(app, "tonemap.wesl");
            embedded_asset!(app, "bloom_optics.wesl");
            embedded_asset!(app, "moonly_lit.wesl");
            if !app.is_plugin_added::<MaterialPlugin<MoonlyLitMaterial>>() {
                app.add_plugins(MaterialPlugin::<MoonlyLitMaterial>::default());
            }
            app.add_systems(
                PostUpdate,
                sync_moonly_lit_materials.in_set(MoonlySystemSet::SyncMaterials),
            );
        }
        app.register_type::<TonemapOperator>()
            .register_type::<Hdr10PostConfig>()
            .register_type::<LuminanceAdaptationState>()
            .register_type::<OpticsMode>()
            .register_type::<MoonlyLitUniform>()
            .init_resource::<Hdr10PostConfig>()
            .init_resource::<LuminanceAdaptationState>()
            .init_resource::<OpticsMode>()
            .add_systems(Update, update_luminance_adaptation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_nine_tonemap_operators_produce_bounded_finite_colors() {
        let operators = [
            TonemapOperator::ClassicXRay,
            TonemapOperator::ExtendedReinhardW2,
            TonemapOperator::ExtendedReinhardW3,
            TonemapOperator::AcesNarkowicz,
            TonemapOperator::AcesHill,
            TonemapOperator::Uchimura,
            TonemapOperator::SteveM,
            TonemapOperator::Uncharted2,
            TonemapOperator::AgX,
        ];
        let hdr = Vec3::new(3.5, 1.8, 0.45);
        for op in operators {
            let mapped = evaluate_tonemap_cpu(hdr, op);
            assert!(mapped.is_finite());
            assert!(mapped.min_element() >= 0.0);
            assert!(mapped.max_element() <= 1.0);
        }
    }

    #[test]
    fn dual_wave_gaussian_weights_sum_to_unity() {
        let w = calc_gauss_dual_wave(0.35);
        let mut total = w.first().copied().unwrap_or(0.0);
        for &tap in w.get(1..).unwrap_or(&[]) {
            total += 2.0 * tap;
        }
        assert!((total - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn karis_downsample_suppresses_extreme_firefly() {
        let firefly = Vec3::splat(1000.0);
        let dark = Vec3::splat(0.1);
        let avg = karis_box4_average(firefly, dark, dark, dark);
        assert!(avg.max_element() < 2.0);
    }
}

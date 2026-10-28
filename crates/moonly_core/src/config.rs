//! Master runtime configuration for the Moonly Moonly-Monolith lighting plugin.

use bevy::{
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
};

/// Default shadow map atlas resolution from `moonly-monolith` (`c_smap_pool_size`).
pub const DEFAULT_SMAP_POOL_SIZE: u32 = 2048;

/// Default near sun cascade split distance in meters (`c_sun_near_distance`).
pub const DEFAULT_SUN_NEAR_METERS: f32 = 20.0;

/// Default middle sun cascade split distance in meters (`c_sun_middle_distance`).
pub const DEFAULT_SUN_MIDDLE_METERS: f32 = 60.0;

/// Default far sun cascade split distance in meters (`c_sun_far_distance`).
pub const DEFAULT_SUN_FAR_METERS: f32 = 180.0;

/// Default sun ray extrusion distance in meters (`c_sun_long_diagonal_offset`).
pub const DEFAULT_SUN_DIAGONAL_OFFSET_METERS: f32 = 600.0;

/// Default photon count per GI bounce (`gi_num_photons`).
pub const DEFAULT_GI_PHOTONS: u32 = 16;

/// Minimum ray hit distance for GI bounce (`gi_clip`).
pub const DEFAULT_GI_CLIP_METERS: f32 = 0.001;

/// Default GI surface reflectivity factor (`ps_r2_gi_refl`).
pub const DEFAULT_GI_REFLECTIVITY: f32 = 0.9;

/// Default reflected-light energy threshold (`r2_gi_clip`).
pub const DEFAULT_GI_ENERGY_CLIP: f32 = 1.0 / 255.0;

/// Minimum virtual light energy threshold (`ps_r2_ls_dlights_clip`).
pub const DEFAULT_DLIGHTS_CLIP: f32 = 0.015;

/// Overall pipeline mode matching `moonly-monolith` tiers or Moonly enhancements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum MoonlyPipelineTier {
    /// Moonly-Monolith R2 CPU lighting and resource baseline.
    ClassicR2,
    /// Moonly-Monolith R3 CPU shadow, GI, and post-processing settings.
    MonolithR3,
    /// Moonly-Monolith R4 CPU shadow, GI, and post-processing settings.
    #[default]
    MonolithR4,
    /// Enhanced Moonly mode with BSSF atlas packing, importance-sampled GI, and PCSS.
    MoonlyEnhanced,
}

/// Shadow filtering mode from `moonly-monolith` and Moonly extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum ShadowFilterMode {
    /// Hardware 2x2 PCF lookup.
    HardwarePcf,
    /// Faithful Moonly-Monolith 64x64 5-rotation jittered shadow filter (`TEX_jitter`).
    #[default]
    MonolithJitter64,
    /// MinMax 4x4 shadow map acceleration (`SE_SUN_NEAR_MINMAX`).
    MonolithMinMax,
    /// Percentage-Closer Soft Shadows with blocker search and contact hardening.
    EnhancedPcss,
}

/// Ambient occlusion mode supported by Moonly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum AmbientOcclusionMode {
    /// Disable ambient occlusion.
    Off,
    /// Classic screen-space ambient occlusion (`phase_ssao`).
    Ssao,
    /// High-Definition Ambient Occlusion (`phase_hdao`).
    Hdao,
    /// Activision S2016 multi-bounce colored AO (`compute_colored_ao`).
    #[default]
    ActivisionColoredAo,
    /// Screen-Space Directional Occlusion with indirect bounce tint.
    EnhancedSsdo,
}

/// Screen-space occlusion sampling and intensity settings.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct AmbientOcclusionSettings {
    /// World-space sampling and rejection radius in meters.
    pub reject_radius: f32,
    /// Minimum accepted occlusion distance in meters.
    pub accept_radius: f32,
    /// Occlusion intensity multiplier.
    pub intensity: f32,
    /// Directional indirect bounce strength for SSDO.
    pub ssdo_bounce_strength: f32,
}

impl Default for AmbientOcclusionSettings {
    fn default() -> Self {
        Self {
            reject_radius: 0.45,
            accept_radius: 0.03,
            intensity: 1.15,
            ssdo_bounce_strength: 0.65,
        }
    }
}

impl ExtractResource<RenderApp> for AmbientOcclusionSettings {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

pub use crate::sun_cascade_config::{SunCascadeConfig, SunCascadeFootprint};

/// Real-time GI radiosity configuration matching `light_GI.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct GiRadiosityConfig {
    /// Number of photon rays cast per shadow-casting light (`gi_num_photons`).
    pub num_photons: u32,
    /// Minimum valid hit distance in meters (`gi_clip`).
    pub clip_distance: f32,
    /// Surface bounce reflectivity multiplier (`ps_r2_gi_refl`).
    pub reflectivity: f32,
    /// Minimum reflected-light energy threshold (`r2_gi_clip`, default `1/255`).
    pub energy_clip: f32,
    /// Normal offset in meters for spawned virtual bounce lights (`0.05` in R4).
    pub normal_standoff: f32,
    /// Enable real-time GI virtual bounce generation.
    pub enabled: bool,
    /// Use cosine-weighted hemisphere importance sampling when in enhanced mode.
    pub cosine_weighted: bool,
}

impl Default for GiRadiosityConfig {
    fn default() -> Self {
        Self {
            num_photons: DEFAULT_GI_PHOTONS,
            clip_distance: DEFAULT_GI_CLIP_METERS,
            reflectivity: DEFAULT_GI_REFLECTIVITY,
            energy_clip: DEFAULT_GI_ENERGY_CLIP,
            normal_standoff: 0.05,
            enabled: true,
            cosine_weighted: false,
        }
    }
}

/// Master resource that controls all Moonly lighting, shadow, GI, and post subsystems.
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct MoonlyConfig {
    /// Active pipeline tier (`ClassicR2`, `MonolithR3`, `MonolithR4`, `MoonlyEnhanced`).
    pub tier: MoonlyPipelineTier,
    /// Shadow map atlas pool resolution (`1536` to `8192`).
    pub smap_pool_size: u32,
    /// Active shadow filter algorithm.
    pub shadow_filter: ShadowFilterMode,
    /// Active ambient occlusion algorithm.
    pub ao_mode: AmbientOcclusionMode,
    /// Cascaded sun shadow settings.
    pub sun_cascades: SunCascadeConfig,
    /// Real-time GI virtual bounce light settings.
    pub gi: GiRadiosityConfig,
    /// Specular glossiness multiplier (`ps_r2_ls_squality`).
    pub specular_quality: f32,
    /// Rain wetness factor in `[0.0, 1.0]`.
    pub rain_wetness: f32,
    /// Wet-surface shadow distances and resolution.
    pub rain_wet_surface: crate::rain_config::RainWetSurfaceConfig,
    /// Local volumetric quality, slice scheduling, and distance fade.
    pub volumetrics: crate::volumetric_config::VolumetricQualityConfig,
    /// Enable software 64x64 hierarchical occlusion rasterizer (`occRasterizer`).
    pub enable_occ_rasterizer: bool,
    /// Enable shadow map caster occlusion pruning (`ps_r2_smapvis`).
    pub enable_smapvis: bool,
}

impl ExtractResource<RenderApp> for MoonlyConfig {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

impl Default for MoonlyConfig {
    fn default() -> Self {
        Self {
            tier: MoonlyPipelineTier::MonolithR4,
            smap_pool_size: DEFAULT_SMAP_POOL_SIZE,
            shadow_filter: ShadowFilterMode::MonolithJitter64,
            ao_mode: AmbientOcclusionMode::ActivisionColoredAo,
            sun_cascades: SunCascadeConfig::default(),
            gi: GiRadiosityConfig::default(),
            specular_quality: 1.0,
            rain_wetness: 0.0,
            rain_wet_surface: crate::rain_config::RainWetSurfaceConfig::default(),
            volumetrics: crate::volumetric_config::VolumetricQualityConfig::default(),
            enable_occ_rasterizer: true,
            enable_smapvis: true,
        }
    }
}

impl MoonlyConfig {
    /// Create a configuration preset for the original Moonly R4 behavior.
    #[must_use]
    pub fn moonly_r4() -> Self {
        Self {
            tier: MoonlyPipelineTier::MonolithR4,
            smap_pool_size: DEFAULT_SMAP_POOL_SIZE,
            shadow_filter: ShadowFilterMode::MonolithJitter64,
            ao_mode: AmbientOcclusionMode::Hdao,
            sun_cascades: SunCascadeConfig::default(),
            gi: GiRadiosityConfig::default(),
            specular_quality: 1.0,
            rain_wetness: 0.0,
            rain_wet_surface: crate::rain_config::RainWetSurfaceConfig::default(),
            volumetrics: crate::volumetric_config::VolumetricQualityConfig::default(),
            enable_occ_rasterizer: true,
            enable_smapvis: true,
        }
    }

    /// Create a configuration preset for the enhanced Moonly mode.
    #[must_use]
    pub fn enhanced() -> Self {
        let mut cfg = Self {
            tier: MoonlyPipelineTier::MoonlyEnhanced,
            smap_pool_size: 4096,
            shadow_filter: ShadowFilterMode::EnhancedPcss,
            ao_mode: AmbientOcclusionMode::EnhancedSsdo,
            ..Self::default()
        };
        cfg.gi.cosine_weighted = true;
        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moonly_r4_preset_uses_original_photon_count() {
        let config = MoonlyConfig::moonly_r4();
        assert_eq!(config.tier, MoonlyPipelineTier::MonolithR4);
        assert_eq!(config.gi.num_photons, 16);
        assert_eq!(config.gi.energy_clip, 1.0 / 255.0);
        assert!(!config.gi.cosine_weighted);
        assert_eq!(config.shadow_filter, ShadowFilterMode::MonolithJitter64);
    }
}

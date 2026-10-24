//! Master runtime configuration for the Moonly XRay-Monolith lighting plugin.

use bevy::prelude::*;

/// Default shadow map atlas resolution from `xray-monolith` (`c_smap_pool_size`).
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
pub const DEFAULT_GI_PHOTONS: u32 = 32;

/// Minimum ray hit distance for GI bounce (`gi_clip`).
pub const DEFAULT_GI_CLIP_METERS: f32 = 0.001;

/// Default GI surface reflectivity factor (`ps_r2_gi_refl`).
pub const DEFAULT_GI_REFLECTIVITY: f32 = 0.9;

/// Minimum virtual light energy threshold (`ps_r2_ls_dlights_clip`).
pub const DEFAULT_DLIGHTS_CLIP: f32 = 0.015;

/// Overall pipeline mode matching `xray-monolith` tiers or Moonly enhancements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum MoonlyPipelineTier {
    /// Faithful XRay-Monolith R2 deferred lighting baseline.
    ClassicR2,
    /// Faithful XRay-Monolith R3 DX10.1 pipeline with MinMax SM and MSAA support.
    MonolithR3,
    /// Faithful XRay-Monolith R4 DX11 pipeline with 32-photon GI, HDAO, and HDR10.
    #[default]
    MonolithR4,
    /// Enhanced Moonly mode with BSSF atlas packing, importance-sampled GI, and PCSS.
    MoonlyEnhanced,
}

/// Shadow filtering mode from `xray-monolith` and Moonly extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum ShadowFilterMode {
    /// Hardware 2x2 PCF lookup.
    HardwarePcf,
    /// Faithful XRay-Monolith 64x64 5-rotation jittered shadow filter (`TEX_jitter`).
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

/// Sun cascade configuration matching `r2_R_sun.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct SunCascadeConfig {
    /// Near cascade split distance in meters (`SE_SUN_NEAR`).
    pub near_split: f32,
    /// Middle cascade split distance in meters (`SE_SUN_MIDDLE`).
    pub middle_split: f32,
    /// Far cascade split distance in meters (`SE_SUN_FAR`).
    pub far_split: f32,
    /// Sun extrusion offset along the light direction (`c_sun_long_diagonal_offset`).
    pub diagonal_offset: f32,
    /// Shadow depth bias scale (`ps_r2_sun_depth_near_bias`).
    pub depth_bias: f32,
    /// Snap cascade projection to texel grid to prevent shimmer.
    pub stabilize_texels: bool,
    /// Enable 4x4 MinMax shadow pyramid for the near cascade.
    pub use_minmax_near: bool,
}

impl Default for SunCascadeConfig {
    fn default() -> Self {
        Self {
            near_split: DEFAULT_SUN_NEAR_METERS,
            middle_split: DEFAULT_SUN_MIDDLE_METERS,
            far_split: DEFAULT_SUN_FAR_METERS,
            diagonal_offset: DEFAULT_SUN_DIAGONAL_OFFSET_METERS,
            depth_bias: -0.00005,
            stabilize_texels: true,
            use_minmax_near: true,
        }
    }
}

/// Real-time GI radiosity configuration matching `light_GI.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct GiRadiosityConfig {
    /// Number of photon rays cast per shadow-casting light (`gi_num_photons`).
    pub num_photons: u32,
    /// Minimum valid hit distance in meters (`gi_clip`).
    pub clip_distance: f32,
    /// Surface bounce reflectivity multiplier (`ps_r2_gi_refl`).
    pub reflectivity: f32,
    /// Minimum virtual light intensity threshold (`ps_r2_ls_dlights_clip`).
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
            energy_clip: DEFAULT_DLIGHTS_CLIP,
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
    /// Enable software 64x64 hierarchical occlusion rasterizer (`occRasterizer`).
    pub enable_occ_rasterizer: bool,
    /// Enable shadow map caster occlusion pruning (`ps_r2_smapvis`).
    pub enable_smapvis: bool,
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
            enable_occ_rasterizer: true,
            enable_smapvis: true,
        }
    }
}

impl MoonlyConfig {
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

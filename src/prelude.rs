//! Unified prelude for the `moonly` Bevy lighting, shadow, GI, and HDR10 plugin.

pub use bevy::prelude::*;
pub use moonly_core::{
    AmbientOcclusionMode, GiRadiosityConfig, HemisphereTracker, LightPackage, MoonlyConfig,
    MoonlyCorePlugin, MoonlyPipelineTier, ShaderBus, ShadowFilterMode, SunCascadeConfig,
    VirtualGiLight, VolumetricLightSettings, WeatherEnvironment, XRayLight, XRayLightFlags,
    XRayLightType,
};
pub use moonly_cull::{
    LightVisibilityState, MoonlyCullPlugin, OccRasterizer, OccTestRect, SmapVisPhase,
    SmapVisTracker,
};
pub use moonly_gi::{
    AmbientOcclusionSettings, GiLightField, GiReflectorBox, MoonlyGiPlugin, compute_colored_ao_cpu,
    evaluate_reflected_gi_at_point, gi_generate_enhanced, gi_generate_monolith,
};
pub use moonly_post::{
    Hdr10PostConfig, LuminanceAdaptationState, MoonlyLitMaterial, MoonlyLitUniform,
    MoonlyPostPlugin, OpticsMode, TonemapOperator, encode_pq_st2084, evaluate_tonemap_cpu,
};
pub use moonly_shadows::{
    CameraFrustumParams, MoonlyShadowsPlugin, RainShadowState, SE_SUN_FAR, SE_SUN_MIDDLE,
    SE_SUN_NEAR, SmapAllocator, SmapRect, SpotShadowXform, SunCascadeSlice, SunCascades,
};
pub use moonly_volumetrics::{
    MoonlyVolumetricsPlugin, SunShaftsConfig, SunShaftsMode, evaluate_volumetric_sunshaft,
    henyey_greenstein_phase,
};

pub use crate::MoonlyPlugin;

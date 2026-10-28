//! Unified prelude for the `moonly` Bevy lighting, shadow, GI, and HDR10 plugin.

pub use bevy::prelude::*;
pub use moonly_core::{
    AmbientOcclusionMode, GiRadiosityConfig, HemisphereTracker, LightPackage, MoonlyConfig,
    MoonlyCorePlugin, MoonlyLight, MoonlyLightFlags, MoonlyLightType, MoonlyNativeAmbient,
    MoonlyNativeForward, MoonlyNativePost, MoonlyPipelineTier, RainWetSurfaceConfig, ShaderBus,
    ShadowFilterMode, SunCascadeConfig, SunCascadeFootprint, VirtualGiLight,
    VolumetricLightSettings, VolumetricQualityConfig, VolumetricSchedule, WeatherEnvironment,
};
pub use moonly_cull::{
    LightVisibilityState, MoonlyCullPlugin, MoonlyHomDiagnostics, MoonlyHomOccluder, MoonlyHomView,
    OccRasterizer, OccTestRect, SmapVisPhase, SmapVisTracker,
};
pub use moonly_gi::{
    AmbientOcclusionSettings, GiLightCache, GiLightField, GiRayGeometry, GiRayHit, GiRayQuery,
    GiReflectorBox, GiTriangle, GiTriangleBvh, MoonlyGiCaster, MoonlyGiPlugin, MoonlyGlow,
    StaticGiGeometry, compute_colored_ao_cpu, evaluate_reflected_gi_at_point, gi_generate_enhanced,
    gi_generate_monolith,
};
pub use moonly_post::{
    Hdr10PostConfig, LuminanceAdaptationState, MoonlyExposureMode, MoonlyLitMaterial,
    MoonlyLitUniform, MoonlyPostPlugin, OpticsMode, TonemapOperator, encode_pq_st2084,
    evaluate_tonemap_cpu,
};
pub use moonly_render::{
    GpuTiming, MoonlyCamera, MoonlyCameraRole, MoonlyCasterVisibility, MoonlyDebugView,
    MoonlyExternalFog, MoonlyForwardView, MoonlyGpuDiagnostics, MoonlyLinearOutput,
    MoonlyMaterialAdapters, MoonlyMaterialLut, MoonlyPass, MoonlyPostSettings, MoonlyRender,
    MoonlyRenderPlugin, MoonlyRenderSystems, MoonlySurface, SetMoonlyForwardBindGroup,
    generate_moonly_material_lut, moonly_forward_layout, moonly_forward_layout_for,
};
pub use moonly_shadows::{
    CameraFrustumParams, LocalShadowBatch, LocalShadowBatches, LocalShadowView,
    MoonlyShadowsPlugin, RainComputeParams, RainShadowState, SE_SUN_FAR, SE_SUN_MIDDLE,
    SE_SUN_NEAR, SmapAllocator, SmapRect, SpotShadowXform, SunCascadeSlice, SunCascades,
};
pub use moonly_volumetrics::{
    MoonlyVolumetricsPlugin, SunShaftsConfig, SunShaftsMode, evaluate_volumetric_sunshaft,
    henyey_greenstein_phase,
};

pub use crate::MoonlyPlugin;

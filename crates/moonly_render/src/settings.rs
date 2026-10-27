use bevy::{
    ecs::query::QueryItem,
    prelude::*,
    render::{RenderApp, extract_component::ExtractComponent, sync_component::SyncComponent},
};
use moonly_post::{Hdr10PostConfig, TonemapOperator};

/// Post-processing parameters for one Moonly camera.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct MoonlyPostSettings {
    /// Tonemapping operator.
    pub operator: TonemapOperator,
    /// Exposure source for this camera.
    pub exposure_mode: moonly_post::MoonlyExposureMode,
    /// Contrast offset around the middle-gray pivot.
    pub contrast: f32,
    /// Saturation multiplier after tonemapping.
    pub saturation: f32,
    /// Display-referred exposure from color grading.
    pub grading_exposure: f32,
    /// Temperature adjustment from color grading.
    pub temperature: f32,
    /// Tint adjustment from color grading.
    pub tint: f32,
    /// Bloom mix weight.
    pub bloom_intensity: f32,
    /// Bloom luminance threshold.
    pub bloom_threshold: f32,
    /// Bloom threshold softness.
    pub bloom_softness: f32,
    /// Procedural lens flare weight.
    pub flare_intensity: f32,
    /// Vignette intensity.
    pub vignette_intensity: f32,
    /// Vignette radius.
    pub vignette_radius: f32,
    /// Vignette edge softness.
    pub vignette_smoothness: f32,
    /// Chromatic aberration strength.
    pub chromatic_aberration: f32,
    /// Peak display luminance in nits.
    pub max_nits: f32,
}

impl Default for MoonlyPostSettings {
    fn default() -> Self {
        Self::from_config(&Hdr10PostConfig::default())
    }
}

impl MoonlyPostSettings {
    #[must_use]
    pub fn from_config(config: &Hdr10PostConfig) -> Self {
        Self {
            operator: config.operator,
            exposure_mode: config.exposure_mode,
            contrast: config.contrast,
            saturation: 1.0 + config.vibrance,
            grading_exposure: 0.0,
            temperature: 0.0,
            tint: 0.0,
            bloom_intensity: config.bloom_intensity,
            bloom_threshold: 1.0,
            bloom_softness: 0.0,
            flare_intensity: config.flare_intensity,
            vignette_intensity: 0.0,
            vignette_radius: 1.0,
            vignette_smoothness: 0.1,
            chromatic_aberration: 0.0,
            max_nits: config.max_nits,
        }
    }

    pub(crate) fn exposure(
        &self,
        camera_exposure: f32,
        config: &Hdr10PostConfig,
        adaptation: &moonly_post::LuminanceAdaptationState,
    ) -> f32 {
        match self.exposure_mode {
            moonly_post::MoonlyExposureMode::Camera => camera_exposure,
            moonly_post::MoonlyExposureMode::LuminanceAdaptation => {
                config.exposure * adaptation.compute_exposure_scale()
            }
        }
    }
}

impl SyncComponent<RenderApp> for MoonlyPostSettings {
    type Target = Self;
}

impl ExtractComponent<RenderApp> for MoonlyPostSettings {
    type QueryData = &'static Self;
    type QueryFilter = With<crate::MoonlyCamera>;
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(*item)
    }
}

#[derive(Component)]
pub(crate) struct MoonlyExposure(pub f32);

impl SyncComponent<RenderApp> for MoonlyExposure {
    type Target = Self;
}

impl ExtractComponent<RenderApp> for MoonlyExposure {
    type QueryData = &'static bevy::camera::Exposure;
    type QueryFilter = With<crate::MoonlyCamera>;
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(Self(item.exposure()))
    }
}

pub(crate) fn configure_view_exposure(
    config: Option<Res<Hdr10PostConfig>>,
    mut cameras: Query<
        (
            &mut bevy::render::camera::ExtractedCamera,
            Option<&MoonlyPostSettings>,
        ),
        With<crate::MoonlyCamera>,
    >,
) {
    let Some(config) = config else {
        return;
    };
    let legacy_exposure = bevy::camera::Exposure::default().exposure();
    for (mut camera, settings) in &mut cameras {
        let mode = settings.map_or(config.exposure_mode, |settings| settings.exposure_mode);
        if mode == moonly_post::MoonlyExposureMode::LuminanceAdaptation {
            camera.exposure = legacy_exposure;
        }
    }
}

pub(crate) fn native_forward_disabled(
    native: Option<Res<moonly_core::MoonlyNativeForward>>,
) -> bool {
    !native.is_some_and(|settings| settings.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_source_selects_adaptation_or_camera_without_cross_scaling() {
        let config = Hdr10PostConfig::default();
        let adaptation = moonly_post::LuminanceAdaptationState::default();
        let mut settings = MoonlyPostSettings::from_config(&config);
        let automatic = settings.exposure(0.37, &config, &adaptation);
        assert!((automatic - config.exposure).abs() < f32::EPSILON);

        settings.exposure_mode = moonly_post::MoonlyExposureMode::Camera;
        let manual = settings.exposure(0.37, &config, &adaptation);
        assert!((manual - 0.37).abs() < f32::EPSILON);
    }
}

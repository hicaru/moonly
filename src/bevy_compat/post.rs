use std::collections::BTreeSet;

use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    post_process::bloom::Bloom,
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
};
use moonly_core::{MoonlyLight, MoonlyLightFlags};
use moonly_post::{MoonlyExposureMode, TonemapOperator};
use moonly_render::{MoonlyCamera, MoonlyPostSettings};

use super::restore_tonemapping;

const DEFAULT_TONEMAP: TonemapOperator = TonemapOperator::AcesHill;

/// Record each unsupported native post option once.
#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct MoonlyBevyCompatWarnings {
    /// Native option names that use a fallback or have no Moonly equivalent.
    pub unsupported_options: BTreeSet<&'static str>,
}

impl MoonlyBevyCompatWarnings {
    fn report(&mut self, option: &'static str) {
        if self.unsupported_options.insert(option) {
            warn!("Moonly Bevy compatibility does not support `{option}`; using a fallback.");
        }
    }
}

#[derive(Component)]
pub(crate) struct ManagedMoonlyPostSettings;

#[derive(Component)]
pub(crate) struct NativePostState {
    pub(crate) tonemapping: Option<Tonemapping>,
}

type NativePostQuery = (
    Option<&'static Bloom>,
    Option<&'static Tonemapping>,
    Option<&'static ColorGrading>,
    &'static mut MoonlyPostSettings,
    &'static mut NativePostState,
);

pub(crate) fn sync_native_post(
    mut warnings: ResMut<MoonlyBevyCompatWarnings>,
    lights: Query<&MoonlyLight>,
    mut cameras: Query<NativePostQuery, With<ManagedMoonlyPostSettings>>,
) {
    let moonly_lighting = lights
        .iter()
        .any(|light| !light.flags.contains(MoonlyLightFlags::NATIVE));
    for (bloom, tonemapping, grading, mut settings, mut state) in &mut cameras {
        if moonly_lighting {
            *settings = MoonlyPostSettings::default();
        } else {
            settings.exposure_mode = MoonlyExposureMode::Camera;
            sync_bloom(bloom, &mut settings, &mut warnings);
            sync_tonemapping(tonemapping, &mut settings, &mut warnings);
            sync_grading(grading, &mut settings, &mut warnings);
        }
        state.tonemapping = tonemapping.copied();
    }
}

pub(crate) fn restore_native_tonemapping(
    mut commands: Commands,
    cameras: Query<(Entity, &NativePostState, Option<&Tonemapping>), Changed<MoonlyCamera>>,
) {
    for (entity, state, current) in &cameras {
        if current.copied() != state.tonemapping {
            restore_tonemapping(&mut commands.entity(entity), state.tonemapping);
        }
    }
}

fn sync_bloom(
    bloom: Option<&Bloom>,
    settings: &mut MoonlyPostSettings,
    warnings: &mut MoonlyBevyCompatWarnings,
) {
    let Some(native) = bloom else {
        let defaults = MoonlyPostSettings::default();
        settings.bloom_intensity = defaults.bloom_intensity;
        settings.bloom_threshold = defaults.bloom_threshold;
        settings.bloom_softness = defaults.bloom_softness;
        return;
    };
    settings.bloom_intensity = native.intensity;
    settings.bloom_threshold = native.prefilter.threshold;
    settings.bloom_softness = native.prefilter.threshold_softness;
    if !settings.native_post && has_unsupported_bloom(native, &Bloom::default()) {
        warnings.report("Bloom advanced shaping and composition");
    }
}

fn has_unsupported_bloom(native: &Bloom, defaults: &Bloom) -> bool {
    native.low_frequency_boost.to_bits() != defaults.low_frequency_boost.to_bits()
        || native.low_frequency_boost_curvature.to_bits()
            != defaults.low_frequency_boost_curvature.to_bits()
        || native.high_pass_frequency.to_bits() != defaults.high_pass_frequency.to_bits()
        || native.composite_mode != defaults.composite_mode
        || native.max_mip_dimension != defaults.max_mip_dimension
        || native.scale != defaults.scale
}

fn sync_tonemapping(
    tonemapping: Option<&Tonemapping>,
    settings: &mut MoonlyPostSettings,
    warnings: &mut MoonlyBevyCompatWarnings,
) {
    let Some(native) = tonemapping.copied() else {
        settings.operator = DEFAULT_TONEMAP;
        return;
    };
    settings.operator = match native {
        Tonemapping::Reinhard => TonemapOperator::ClassicMoonly,
        Tonemapping::AcesFitted => TonemapOperator::AcesHill,
        Tonemapping::AgX => TonemapOperator::AgX,
        _ => {
            if !settings.native_post {
                warnings.report(tonemapping_name(native));
            }
            DEFAULT_TONEMAP
        }
    };
}

const fn tonemapping_name(tonemapping: Tonemapping) -> &'static str {
    match tonemapping {
        Tonemapping::None => "Tonemapping::None",
        Tonemapping::Linear => "Tonemapping::Linear",
        Tonemapping::ReinhardLuminance => "Tonemapping::ReinhardLuminance",
        Tonemapping::SomewhatBoringDisplayTransform => {
            "Tonemapping::SomewhatBoringDisplayTransform"
        }
        Tonemapping::TonyMcMapface => "Tonemapping::TonyMcMapface",
        Tonemapping::BlenderFilmic => "Tonemapping::BlenderFilmic",
        Tonemapping::KhronosPbrNeutral => "Tonemapping::KhronosPbrNeutral",
        Tonemapping::Reinhard | Tonemapping::AcesFitted | Tonemapping::AgX => "",
    }
}

fn sync_grading(
    grading: Option<&ColorGrading>,
    settings: &mut MoonlyPostSettings,
    warnings: &mut MoonlyBevyCompatWarnings,
) {
    let defaults = ColorGradingGlobal::default();
    let global = grading.map_or(&defaults, |grading| &grading.global);
    settings.grading_exposure = global.exposure;
    settings.temperature = global.temperature;
    settings.tint = global.tint;
    settings.saturation = global.post_saturation;
    let Some(grading) = grading else {
        return;
    };
    if !settings.native_post {
        report_unsupported_grading(grading, &defaults, warnings);
    }
}

fn report_unsupported_grading(
    grading: &ColorGrading,
    defaults: &ColorGradingGlobal,
    warnings: &mut MoonlyBevyCompatWarnings,
) {
    if grading.global.hue != 0.0 {
        warnings.report("ColorGradingGlobal::hue");
    }
    if grading.global.midtones_range != defaults.midtones_range {
        warnings.report("ColorGradingGlobal::midtones_range");
    }
    if grading
        .all_sections()
        .any(|section| *section != ColorGradingSection::default())
    {
        warnings.report("ColorGrading sectional adjustments");
    }
}

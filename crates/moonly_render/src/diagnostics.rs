use std::time::Duration;

use bevy::prelude::*;

/// Timestamp scope for one Moonly render pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum MoonlyPass {
    GBuffer,
    ShadowLocal,
    ShadowSun,
    AccumDirect,
    AccumLocal,
    AccumGi,
    AmbientOcclusion,
    Volumetrics,
    Combine,
    Post,
    Bloom,
    Luminance,
}

impl MoonlyPass {
    pub const ALL: [Self; 12] = [
        Self::GBuffer,
        Self::ShadowLocal,
        Self::ShadowSun,
        Self::AccumDirect,
        Self::AccumLocal,
        Self::AccumGi,
        Self::AmbientOcclusion,
        Self::Volumetrics,
        Self::Combine,
        Self::Post,
        Self::Bloom,
        Self::Luminance,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::GBuffer => 0,
            Self::ShadowLocal => 1,
            Self::ShadowSun => 2,
            Self::AccumDirect => 3,
            Self::AccumLocal => 4,
            Self::AccumGi => 5,
            Self::AmbientOcclusion => 6,
            Self::Volumetrics => 7,
            Self::Combine => 8,
            Self::Post => 9,
            Self::Bloom => 10,
            Self::Luminance => 11,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GBuffer => "gbuffer",
            Self::ShadowLocal => "shadow_local",
            Self::ShadowSun => "shadow_sun",
            Self::AccumDirect => "accum_direct",
            Self::AccumLocal => "accum_local",
            Self::AccumGi => "accum_gi",
            Self::AmbientOcclusion => "ao",
            Self::Volumetrics => "volumetrics",
            Self::Combine => "combine",
            Self::Post => "post",
            Self::Bloom => "bloom",
            Self::Luminance => "luminance",
        }
    }
}

/// Availability and value of a GPU timestamp result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GpuTiming {
    /// The adapter or render path cannot provide this result.
    #[default]
    Unavailable,
    /// The GPU has not returned this result yet.
    Pending,
    /// A completed GPU duration.
    Available(Duration),
}

/// Latest GPU timing state for all Moonly passes.
#[derive(Resource, Debug, Clone)]
pub struct MoonlyGpuDiagnostics {
    timings: [GpuTiming; MoonlyPass::ALL.len()],
}

impl Default for MoonlyGpuDiagnostics {
    fn default() -> Self {
        Self {
            timings: [GpuTiming::Unavailable; MoonlyPass::ALL.len()],
        }
    }
}

impl MoonlyGpuDiagnostics {
    #[must_use]
    pub fn timing(&self, pass: MoonlyPass) -> GpuTiming {
        self.timings.get(pass.index()).copied().unwrap_or_default()
    }

    pub fn set_timing(&mut self, pass: MoonlyPass, timing: GpuTiming) {
        if let Some(slot) = self.timings.get_mut(pass.index()) {
            *slot = timing;
        }
    }

    #[must_use]
    pub fn completed_total(&self) -> Option<Duration> {
        self.timings
            .iter()
            .try_fold(Duration::ZERO, |total, timing| match timing {
                GpuTiming::Available(duration) => Some(total + *duration),
                GpuTiming::Unavailable | GpuTiming::Pending => None,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_and_pending_never_become_zero_duration() {
        let mut diagnostics = MoonlyGpuDiagnostics::default();
        assert_eq!(diagnostics.completed_total(), None);
        diagnostics.set_timing(MoonlyPass::GBuffer, GpuTiming::Pending);
        assert_eq!(diagnostics.completed_total(), None);
    }

    #[test]
    fn total_requires_every_pass() {
        let mut diagnostics = MoonlyGpuDiagnostics::default();
        for pass in MoonlyPass::ALL {
            diagnostics.set_timing(pass, GpuTiming::Available(Duration::from_micros(10)));
        }
        assert_eq!(
            diagnostics.completed_total(),
            Some(Duration::from_micros(120))
        );
    }
}

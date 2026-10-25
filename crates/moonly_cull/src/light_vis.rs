//! Light visibility preparation and frame-staggered occlusion scheduling (`light_vis.cpp`).

use bevy::prelude::*;
use moonly_core::{MoonlyLight, MoonlyLightType};

/// World-space coordinate safety bound from `light::vis_prepare` (`10000.f`).
pub const SAFE_WORLD_BOUND_METERS: f32 = 10_000.0;

/// Cone safety margin multiplier from `light_vis.cpp` (`0.7f`).
pub const EYE_IN_CONE_SCALE: f32 = 0.7;

/// Cone safety margin bias in radians from `light_vis.cpp` (`0.05f`).
pub const EYE_IN_CONE_BIAS_RAD: f32 = 0.05;

/// Visibility state for a dynamic light ported from `light::vis` (`light.h`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct LightVisibilityState {
    /// Whether the light is currently visible (`vis.visible`).
    pub visible: bool,
    /// Whether an occlusion query is currently in flight (`vis.pending`).
    pub pending: bool,
    /// Next frame index when an occlusion check should run (`vis.frame2test`).
    pub frame_to_test: u32,
    /// Deterministic hash seed for frame-interval jitter.
    pub jitter_seed: u32,
}

impl Default for LightVisibilityState {
    fn default() -> Self {
        Self {
            visible: true,
            pending: false,
            frame_to_test: 0,
            jitter_seed: 1,
        }
    }
}

impl LightVisibilityState {
    /// Determine whether the camera eye lies inside the light volume (`light::vis_prepare`).
    #[must_use]
    pub fn is_eye_inside_light(light: &MoonlyLight, eye_pos: Vec3) -> bool {
        if !in_safe_world_bounds(eye_pos) {
            return false;
        }
        let to_eye = eye_pos - light.position;
        let dist_sq = to_eye.length_squared();
        if dist_sq >= light.range * light.range {
            return false;
        }
        match light.light_type {
            MoonlyLightType::Point | MoonlyLightType::Direct => true,
            MoonlyLightType::Spot | MoonlyLightType::Omnipart => {
                let dir = to_eye.normalize_or_zero();
                let threshold = (light.cone_angle * EYE_IN_CONE_SCALE + EYE_IN_CONE_BIAS_RAD).cos();
                dir.dot(light.direction) >= threshold
            }
        }
    }

    /// Prepare light visibility before issuing an occlusion test (`light::vis_prepare`).
    /// Returns `true` when an occlusion test should be submitted this frame.
    pub fn vis_prepare(&mut self, light: &MoonlyLight, eye_pos: Vec3, current_frame: u32) -> bool {
        if Self::is_eye_inside_light(light, eye_pos) {
            self.visible = true;
            self.pending = false;
            return false;
        }
        if current_frame < self.frame_to_test {
            return false;
        }
        self.pending = true;
        true
    }

    /// Consume the occlusion fragment count and schedule the next test frame (`light::vis_update`).
    pub const fn vis_update(&mut self, visible_fragments: u32, current_frame: u32) {
        if !self.pending {
            return;
        }
        self.pending = false;
        self.visible = visible_fragments > 0;
        self.jitter_seed = self
            .jitter_seed
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let delay = if self.visible {
            1 + (self.jitter_seed % 10)
        } else {
            30 + (self.jitter_seed % 71)
        };
        self.frame_to_test = current_frame.saturating_add(delay);
    }
}

fn in_safe_world_bounds(p: Vec3) -> bool {
    p.x.abs() <= SAFE_WORLD_BOUND_METERS
        && p.y.abs() <= SAFE_WORLD_BOUND_METERS
        && p.z.abs() <= SAFE_WORLD_BOUND_METERS
}

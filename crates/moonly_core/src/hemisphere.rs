//! Hemisphere, sun, and skybox ambient tracking ported from `LightTrack.h` and `LightTrack.cpp`.

use std::f32::consts::FRAC_1_SQRT_2;

use bevy::prelude::*;

/// Rising rate multiplier for temporal light adaptation (`f_inc = 4.0` in `LightTrack.cpp`).
pub const HEMI_RISE_SPEED: f32 = 4.0;

/// Falling rate multiplier for temporal light adaptation (`f_dec = 0.5` in `LightTrack.cpp`).
pub const HEMI_FALL_SPEED: f32 = 0.5;

/// Number of hemisphere visibility sample rays (`lt_hemisamples` in `LightTrack.h`).
pub const HEMI_SAMPLE_COUNT: usize = 15;

/// Six cardinal directions for `hemi_cube` skybox probe tracking (`LightTrack.cpp`).
pub const HEMI_CUBE_DIRS: [Vec3; 6] = [
    Vec3::X,
    Vec3::NEG_X,
    Vec3::Y,
    Vec3::NEG_Y,
    Vec3::Z,
    Vec3::NEG_Z,
];

/// Fifteen deterministic upper-hemisphere sample directions for sky occlusion.
pub const HEMI_SAMPLE_DIRS: [Vec3; HEMI_SAMPLE_COUNT] = [
    Vec3::new(0.0, 1.0, 0.0),
    Vec3::new(0.577_35, 0.577_35, 0.577_35),
    Vec3::new(-0.577_35, 0.577_35, 0.577_35),
    Vec3::new(0.577_35, 0.577_35, -0.577_35),
    Vec3::new(-0.577_35, 0.577_35, -0.577_35),
    Vec3::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2, 0.0),
    Vec3::new(-FRAC_1_SQRT_2, FRAC_1_SQRT_2, 0.0),
    Vec3::new(0.0, FRAC_1_SQRT_2, FRAC_1_SQRT_2),
    Vec3::new(0.0, FRAC_1_SQRT_2, -FRAC_1_SQRT_2),
    Vec3::new(0.301_511, 0.904_534, 0.301_511),
    Vec3::new(-0.301_511, 0.904_534, 0.301_511),
    Vec3::new(0.301_511, 0.904_534, -0.301_511),
    Vec3::new(-0.301_511, 0.904_534, -0.301_511),
    Vec3::new(0.866_025, 0.5, 0.0),
    Vec3::new(0.0, 0.5, 0.866_025),
];

/// Global weather and atmosphere lighting resource matching `xray-monolith` globals.
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct WeatherEnvironment {
    /// Hemisphere sky ambient color and intensity (`L_hemi_color`).
    pub hemi_color: Vec4,
    /// Direct sun color in linear RGB (`L_sun_color`).
    pub sun_color: Vec3,
    /// Normalized direction toward the sun (`-L_sun_dir_w`).
    pub sun_direction: Vec3,
    /// Base ambient floor color (`L_ambient`).
    pub ambient_color: Vec3,
    /// Distance fog color (`fog_color`).
    pub fog_color: Vec3,
    /// Plane equation for `calc_fogging(w_pos)` in `common_functions.h` (`fog_plane`).
    pub fog_plane: Vec4,
    /// Skybox 6-face irradiance cube values (`hemi_cube`).
    pub sky_cube: [f32; 6],
}

impl Default for WeatherEnvironment {
    fn default() -> Self {
        Self {
            hemi_color: Vec4::new(0.42, 0.50, 0.64, 0.85),
            sun_color: Vec3::new(1.35, 1.18, 0.95),
            sun_direction: Vec3::new(-0.45, -0.78, -0.43).normalize(),
            ambient_color: Vec3::new(0.06, 0.07, 0.09),
            fog_color: Vec3::new(0.36, 0.42, 0.50),
            fog_plane: Vec4::new(0.0, -0.002, 0.0, 0.02),
            sky_cube: [0.65, 0.55, 1.0, 0.25, 0.60, 0.58],
        }
    }
}

impl WeatherEnvironment {
    /// Evaluate distance and height fog factor `dot(vec4(w_pos, 1.0), fog_plane)` (`common_functions.h`).
    #[must_use]
    pub fn calc_fogging(&self, world_pos: Vec3) -> f32 {
        world_pos.extend(1.0).dot(self.fog_plane).clamp(0.0, 1.0)
    }
}

/// Per-entity hemisphere and sun occlusion tracker ported from `CROS_impl` (`LightTrack.h`).
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct HemisphereTracker {
    /// Target unsmoothed hemisphere occlusion value (`hemi_value`).
    pub hemi_value: f32,
    /// Smoothed hemisphere occlusion value (`hemi_smooth`).
    pub hemi_smooth: f32,
    /// Target unsmoothed sun visibility value (`sun_value`).
    pub sun_value: f32,
    /// Smoothed sun visibility value (`sun_smooth`).
    pub sun_smooth: f32,
    /// Target 6-face skybox cube factors (`hemi_cube`).
    pub hemi_cube: [f32; 6],
    /// Smoothed 6-face skybox cube factors (`hemi_cube_smooth`).
    pub hemi_cube_smooth: [f32; 6],
    /// Accumulated RGB illumination from nearby dynamic point/spot lights (`approximate`).
    pub approximate_local: Vec3,
}

impl Default for HemisphereTracker {
    fn default() -> Self {
        Self {
            hemi_value: 0.85,
            hemi_smooth: 0.85,
            sun_value: 1.0,
            sun_smooth: 1.0,
            hemi_cube: [0.65, 0.55, 1.0, 0.25, 0.60, 0.58],
            hemi_cube_smooth: [0.65, 0.55, 1.0, 0.25, 0.60, 0.58],
            approximate_local: Vec3::ZERO,
        }
    }
}

impl HemisphereTracker {
    /// Smooth a single channel with asymmetric rise/fall speed from `LightTrack.cpp`.
    #[must_use]
    pub fn smooth_value(current: f32, target: f32, dt: f32) -> f32 {
        let rate = if target > current {
            HEMI_RISE_SPEED
        } else {
            HEMI_FALL_SPEED
        };
        let factor = (dt * rate).clamp(0.0, 1.0);
        current * (1.0 - factor) + target * factor
    }

    /// Advance `hemi_smooth`, `sun_smooth`, and `hemi_cube_smooth` by `dt` seconds.
    pub fn step_smoothing(&mut self, dt: f32) {
        self.hemi_smooth = Self::smooth_value(self.hemi_smooth, self.hemi_value, dt);
        self.sun_smooth = Self::smooth_value(self.sun_smooth, self.sun_value, dt);
        for (smooth, target) in self.hemi_cube_smooth.iter_mut().zip(self.hemi_cube.iter()) {
            *smooth = Self::smooth_value(*smooth, *target, dt);
        }
    }

    /// Evaluate combined material ambient and hemisphere irradiance for a surface normal.
    #[must_use]
    pub fn evaluate_irradiance(&self, normal: Vec3, env: &WeatherEnvironment) -> Vec3 {
        let mut cube_sum = 0.0_f32;
        for (dir, weight) in HEMI_CUBE_DIRS.iter().zip(self.hemi_cube_smooth.iter()) {
            let ndotl = normal.dot(*dir).max(0.0);
            cube_sum += ndotl * (*weight);
        }
        let hemi_rgb = env.hemi_color.truncate() * (self.hemi_smooth * 0.5 + cube_sum * 0.25);
        let sun_ndotl = normal.dot(-env.sun_direction).max(0.0);
        let sun_rgb = env.sun_color * (sun_ndotl * self.sun_smooth);
        env.ambient_color + hemi_rgb + sun_rgb + self.approximate_local
    }
}

/// Bevy system that advances all `HemisphereTracker` components each frame.
pub fn update_hemisphere_trackers(
    time: Res<Time>,
    env: Res<WeatherEnvironment>,
    mut query: Query<&mut HemisphereTracker>,
) {
    let dt = time.delta_secs();
    for mut tracker in &mut query {
        tracker.hemi_cube = env.sky_cube;
        tracker.step_smoothing(dt);
    }
}

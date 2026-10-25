//! Hemisphere, sun, and skybox ambient tracking ported from `LightTrack.h` and `LightTrack.cpp`.

use bevy::{
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
};

/// Rising rate multiplier for temporal light adaptation.
pub const HEMI_RISE_SPEED: f32 = 4.0;

/// Falling rate multiplier for temporal light adaptation.
pub const HEMI_FALL_SPEED: f32 = 2.0;

/// Number of distributed visibility samples.
pub const HEMI_SAMPLE_COUNT: usize = 26;

/// Cube faces use the order positive X, Y, Z, then negative X, Y, Z.
pub const HEMI_CUBE_DIRS: [Vec3; 6] = [
    Vec3::X,
    Vec3::Y,
    Vec3::Z,
    Vec3::NEG_X,
    Vec3::NEG_Y,
    Vec3::NEG_Z,
];

const EDGE: f32 = std::f32::consts::FRAC_1_SQRT_2;
const CORNER: f32 = 0.577_350_26;

/// Deterministic axis, edge, and corner directions over the full sphere.
pub const HEMI_SAMPLE_DIRS: [Vec3; HEMI_SAMPLE_COUNT] = [
    Vec3::X,
    Vec3::Y,
    Vec3::Z,
    Vec3::NEG_X,
    Vec3::NEG_Y,
    Vec3::NEG_Z,
    Vec3::new(EDGE, EDGE, 0.0),
    Vec3::new(EDGE, NEG_EDGE, 0.0),
    Vec3::new(EDGE, 0.0, EDGE),
    Vec3::new(EDGE, 0.0, NEG_EDGE),
    Vec3::new(NEG_EDGE, EDGE, 0.0),
    Vec3::new(NEG_EDGE, NEG_EDGE, 0.0),
    Vec3::new(NEG_EDGE, 0.0, EDGE),
    Vec3::new(NEG_EDGE, 0.0, NEG_EDGE),
    Vec3::new(0.0, EDGE, EDGE),
    Vec3::new(0.0, EDGE, NEG_EDGE),
    Vec3::new(0.0, NEG_EDGE, EDGE),
    Vec3::new(0.0, NEG_EDGE, NEG_EDGE),
    Vec3::new(CORNER, CORNER, CORNER),
    Vec3::new(CORNER, CORNER, -CORNER),
    Vec3::new(CORNER, -CORNER, CORNER),
    Vec3::new(CORNER, -CORNER, -CORNER),
    Vec3::new(-CORNER, CORNER, CORNER),
    Vec3::new(-CORNER, CORNER, -CORNER),
    Vec3::new(-CORNER, -CORNER, CORNER),
    Vec3::new(-CORNER, -CORNER, -CORNER),
];

const NEG_EDGE: f32 = -EDGE;

/// Global weather and atmosphere lighting resource matching `moonly-monolith` globals.
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
    /// Camera-space fog start in meters (`fog_near`).
    pub fog_near: f32,
    /// Camera-space fog end in meters (`fog_far`).
    pub fog_far: f32,
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
            fog_near: 0.0,
            fog_far: 200.0,
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

    /// Compute the camera-dependent R4 fog plane from a world-to-clip matrix.
    #[must_use]
    pub fn fog_plane_for_view(&self, clip_from_world: Mat4) -> Vec4 {
        let plane = Vec4::new(
            -(clip_from_world.x_axis.w + clip_from_world.x_axis.z),
            -(clip_from_world.y_axis.w + clip_from_world.y_axis.z),
            -(clip_from_world.z_axis.w + clip_from_world.z_axis.z),
            -(clip_from_world.w_axis.w + clip_from_world.w_axis.z),
        );
        let plane = plane / plane.truncate().length().max(1.0e-6);
        let distance_scale = 1.0 / (self.fog_far - self.fog_near).max(1.0e-3);
        Vec4::new(
            -plane.x * distance_scale,
            -plane.y * distance_scale,
            -plane.z * distance_scale,
            (-plane.w - self.fog_near) * distance_scale,
        )
    }
}

impl ExtractResource<RenderApp> for WeatherEnvironment {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Per-entity ambient, sun, and local-light visibility tracker.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct HemisphereTracker {
    /// Target unsmoothed hemisphere visibility.
    pub hemi_value: f32,
    /// Smoothed hemisphere visibility.
    pub hemi_smooth: f32,
    /// Target unsmoothed sun visibility.
    pub sun_value: f32,
    /// Smoothed sun visibility.
    pub sun_smooth: f32,
    /// Target six-face visibility factors.
    pub hemi_cube: [f32; 6],
    /// Smoothed six-face visibility factors.
    pub hemi_cube_smooth: [f32; 6],
    /// Accumulated RGB illumination from nearby local lights.
    pub approximate_local: Vec3,
    /// Incremental sample energies. Use `record_sample` to update them.
    #[doc(hidden)]
    pub sample_energy: [f32; HEMI_SAMPLE_COUNT],
    /// Next distributed sample index.
    #[doc(hidden)]
    pub sample_cursor: usize,
}

impl Default for HemisphereTracker {
    fn default() -> Self {
        Self {
            hemi_value: 1.0,
            hemi_smooth: 1.0,
            sun_value: 1.0,
            sun_smooth: 1.0,
            hemi_cube: [1.0; 6],
            hemi_cube_smooth: [1.0; 6],
            approximate_local: Vec3::ZERO,
            sample_energy: [1.0; HEMI_SAMPLE_COUNT],
            sample_cursor: 0,
        }
    }
}

impl HemisphereTracker {
    /// Smooth one channel with asymmetric rise and fall rates.
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

    /// Advance all smoothed visibility channels by `dt` seconds.
    pub fn step_smoothing(&mut self, dt: f32) {
        self.hemi_smooth = Self::smooth_value(self.hemi_smooth, self.hemi_value, dt);
        self.sun_smooth = Self::smooth_value(self.sun_smooth, self.sun_value, dt);
        for (smooth, target) in self.hemi_cube_smooth.iter_mut().zip(self.hemi_cube.iter()) {
            *smooth = Self::smooth_value(*smooth, *target, dt);
        }
    }

    /// Return the next sample index and direction.
    pub fn next_sample(&mut self) -> Option<(usize, Vec3)> {
        let index = self.sample_cursor % HEMI_SAMPLE_COUNT;
        self.sample_cursor = self.sample_cursor.wrapping_add(1);
        HEMI_SAMPLE_DIRS.get(index).copied().map(|dir| (index, dir))
    }

    /// Blend one visibility result into its persistent sample energy.
    pub fn record_sample(&mut self, index: usize, visibility: f32) {
        let Some(energy) = self.sample_energy.get_mut(index) else {
            return;
        };
        let test = visibility.clamp(-0.5, 1.0);
        *energy = (*energy * 0.9 + test * 0.1).clamp(-0.5, 1.0);
        self.refresh_targets();
    }

    /// Evaluate ambient irradiance without duplicating direct sun accumulation.
    #[must_use]
    pub fn evaluate_irradiance(&self, normal: Vec3, env: &WeatherEnvironment) -> Vec3 {
        let cube_sum = HEMI_CUBE_DIRS
            .iter()
            .zip(self.hemi_cube_smooth.iter())
            .map(|(dir, weight)| normal.dot(*dir).max(0.0) * weight.max(1.0 / 255.0))
            .sum::<f32>();
        let hemi = env.hemi_color.truncate()
            * env.hemi_color.w
            * (self.hemi_smooth * 0.5 + cube_sum * 0.25)
            * 0.75;
        env.ambient_color + hemi + self.approximate_local
    }

    fn refresh_targets(&mut self) {
        let hemi_norm = directional_weight(Vec3::Y);
        self.hemi_value = weighted_energy(Vec3::Y, &self.sample_energy) / hemi_norm;
        for (face, target) in HEMI_CUBE_DIRS.iter().zip(self.hemi_cube.iter_mut()) {
            *target = weighted_energy(*face, &self.sample_energy) / directional_weight(*face);
        }
    }
}

fn directional_weight(direction: Vec3) -> f32 {
    HEMI_SAMPLE_DIRS
        .iter()
        .map(|sample| sample.dot(direction).max(0.0))
        .sum()
}

fn weighted_energy(direction: Vec3, energies: &[f32; HEMI_SAMPLE_COUNT]) -> f32 {
    HEMI_SAMPLE_DIRS
        .iter()
        .zip(energies.iter())
        .map(|(sample, energy)| sample.dot(direction).max(0.0) * energy)
        .sum()
}

/// Advance temporal smoothing for all trackers.
pub fn update_hemisphere_trackers(time: Res<Time>, mut query: Query<&mut HemisphereTracker>) {
    let dt = time.delta_secs();
    for mut tracker in &mut query {
        tracker.step_smoothing(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fog_plane_tracks_camera_depth_and_clamps_range() {
        let mut environment = WeatherEnvironment {
            fog_near: 0.0,
            fog_far: 100.0,
            ..WeatherEnvironment::default()
        };
        let projection = crate::perspective_rh_mat4(std::f32::consts::FRAC_PI_2, 1.0, 0.1, 200.0);
        environment.fog_plane = environment.fog_plane_for_view(projection);
        for (position, expected) in [
            (Vec3::ZERO, 0.0),
            (Vec3::new(0.0, 0.0, -50.0), 0.5),
            (Vec3::new(0.0, 0.0, -100.0), 1.0),
        ] {
            let actual = environment.calc_fogging(position);
            assert!((actual - expected).abs() < 0.01, "{actual} != {expected}");
        }
    }
}

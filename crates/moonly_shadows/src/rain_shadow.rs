//! Orthographic wet-surface rain shadow calculation ported from `r4_R_rain.cpp`.

use bevy::prelude::*;

use crate::spot_xform::{look_to_rh_mat4, orthographic_rh_mat4};

/// Rain cloud virtual source distance (`source_offset = 10000.f` in `r4_R_rain.cpp`).
pub const RAIN_SOURCE_OFFSET_METERS: f32 = 10_000.0;

/// Center-of-projection offset for rain caster culling (`cull_COP_offs = 1200.f`).
pub const RAIN_CULL_COP_OFFSET_METERS: f32 = 1_200.0;

/// Orthographic shadow frustum pullback (`ortho_xform_offs = 1000.f`).
pub const RAIN_ORTHO_XFORM_OFFSET_METERS: f32 = 1_000.0;

/// Default maximum rain shadow distance in meters (`max_distance = 45.0f`).
pub const DEFAULT_RAIN_MAX_DISTANCE_METERS: f32 = 45.0;

/// Computed wet-surface rain shadow frustum and matrix (`r4_R_rain.cpp`).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct RainShadowState {
    /// Rain direction vector (normalized, pointing downward).
    pub rain_direction: Vec3,
    /// Bounding sphere center for rain shadow casters.
    pub sphere_center: Vec3,
    /// Bounding sphere radius `R = H^2 (1 + a^2 + c^2) / (2H)`.
    pub sphere_radius: f32,
    /// Rain light-space view matrix.
    pub view: Mat4,
    /// Rain light-space orthographic projection matrix.
    pub proj: Mat4,
    /// Combined `proj * view` rain shadow matrix.
    pub combine: Mat4,
}

impl Default for RainShadowState {
    fn default() -> Self {
        Self {
            rain_direction: Vec3::new(0.08, -0.99, 0.05).normalize(),
            sphere_center: Vec3::ZERO,
            sphere_radius: DEFAULT_RAIN_MAX_DISTANCE_METERS,
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
            combine: Mat4::IDENTITY,
        }
    }
}

impl RainShadowState {
    /// Compute the bounding sphere and orthographic rain shadow matrix (`CRender::render_rain`).
    #[must_use]
    pub fn compute(
        camera_pos: Vec3,
        camera_dir: Vec3,
        rain_dir: Vec3,
        fov_y_rad: f32,
        aspect: f32,
    ) -> Self {
        let h = DEFAULT_RAIN_MAX_DISTANCE_METERS;
        let a = (0.5 * fov_y_rad).tan();
        let c = a * aspect;
        let radius = (h * h * (1.0 + a * a + c * c)) / (2.0 * h);
        let center = camera_pos + camera_dir.normalize_or_zero() * (h - radius);

        let r_dir = rain_dir.normalize_or_zero();
        let up_ref = if r_dir.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let eye = center - r_dir * RAIN_ORTHO_XFORM_OFFSET_METERS;
        let view = look_to_rh_mat4(eye, r_dir, up_ref);
        let proj = orthographic_rh_mat4(
            (-radius, radius, -radius, radius),
            (1.0, RAIN_ORTHO_XFORM_OFFSET_METERS + 2.0 * radius),
        );
        let combine = proj * view;

        Self {
            rain_direction: r_dir,
            sphere_center: center,
            sphere_radius: radius,
            view,
            proj,
            combine,
        }
    }
}

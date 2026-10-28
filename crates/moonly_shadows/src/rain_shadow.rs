//! Orthographic wet-surface rain shadow calculation ported from `r4_R_rain.cpp`.

use bevy::prelude::*;
use moonly_core::RainWetSurfaceConfig;

use crate::{
    CameraFrustumParams,
    spot_xform::{look_to_rh_mat4, orthographic_rh_mat4},
};

/// Rain cloud virtual source distance (`source_offset = 10000.f` in `r4_R_rain.cpp`).
pub const RAIN_SOURCE_OFFSET_METERS: f32 = 10_000.0;
/// Center-of-projection offset for rain caster culling (`cull_COP_offs = 1200.f`).
pub const RAIN_CULL_COP_OFFSET_METERS: f32 = 1_200.0;
/// Orthographic shadow frustum pullback (`ortho_xform_offs = 1000.f`).
pub const RAIN_ORTHO_XFORM_OFFSET_METERS: f32 = 1_000.0;
/// Default maximum rain shadow distance in meters (`ps_r3_dyn_wet_surf_far`).
pub const DEFAULT_RAIN_MAX_DISTANCE_METERS: f32 = 30.0;
const REFERENCE_RADIUS_SCALE: f32 = 1.05;

/// Camera and configuration values for one rain shadow update.
#[derive(Debug, Clone, Copy)]
pub struct RainComputeParams {
    /// Active camera frustum.
    pub camera: CameraFrustumParams,
    /// Normalized rain fall direction.
    pub rain_direction: Vec3,
    /// Wet-surface distances and requested resolution.
    pub config: RainWetSurfaceConfig,
    /// Available shadow pool resolution.
    pub pool_size: u32,
}

/// Computed wet-surface rain shadow frustum and matrix (`r4_R_rain.cpp`).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct RainShadowState {
    /// Rain direction vector (normalized, pointing downward).
    pub rain_direction: Vec3,
    /// Bounding sphere center for rain shadow casters.
    pub sphere_center: Vec3,
    /// Bounding sphere radius with the R4 five-percent margin.
    pub sphere_radius: f32,
    /// Rain light-space view matrix.
    pub view: Mat4,
    /// Rain light-space orthographic projection matrix.
    pub proj: Mat4,
    /// Texel-snapped `proj * view` rain shadow matrix.
    pub combine: Mat4,
    /// Effective resolution after pool and configuration limits.
    pub smap_size: u32,
    /// Number of completed rain shadow updates.
    pub generation: u64,
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
            smap_size: RainWetSurfaceConfig::default().smap_resolution,
            generation: 0,
        }
    }
}

impl RainShadowState {
    /// Compute the R4 footprint, asymmetric depth range, and snapped matrix.
    #[must_use]
    pub fn compute(params: RainComputeParams) -> Self {
        let camera = params.camera;
        let camera_position = camera.camera_transform.translation;
        let camera_direction = camera.camera_transform.forward().as_vec3();
        let rain_direction = params.rain_direction.normalize_or_zero();
        let rain_direction = if rain_direction == Vec3::ZERO {
            Vec3::NEG_Y
        } else {
            rain_direction
        };
        let radius = bounding_radius(
            camera.fov_y_radians,
            camera.aspect_ratio,
            params.config.far_meters(),
        );
        let center = footprint_center(camera_position, camera_direction, rain_direction, radius);
        let eye = camera_position - rain_direction * RAIN_SOURCE_OFFSET_METERS;
        let view = look_to_rh_mat4(eye, rain_direction, rain_up(rain_direction));
        let (min_ls, max_ls) = light_space_bounds(&camera, params.config.far_meters(), view);
        let near_far = light_space_depth_range(min_ls.z, max_ls.z);
        let footprint_offset = footprint_offset(camera_direction, radius);
        let offset = view.transform_vector3(footprint_offset).truncate();
        let proj = orthographic_rh_mat4(
            (
                -radius + offset.x,
                radius + offset.x,
                -radius + offset.y,
                radius + offset.y,
            ),
            near_far,
        );
        let smap_size = params.config.effective_smap_size(params.pool_size);
        let combine = snap_to_texel(proj * view, smap_size, params.pool_size);
        Self {
            rain_direction,
            sphere_center: center,
            sphere_radius: radius,
            view,
            proj,
            combine,
            smap_size,
            generation: 1,
        }
    }
}

fn bounding_radius(fov_y_rad: f32, aspect: f32, far_meters: f32) -> f32 {
    let a = (0.5 * fov_y_rad).tan();
    let c = (0.5 * fov_y_rad * aspect).tan();
    let radius = far_meters * (1.0 + a * a + c * c) * 0.5;
    (radius * REFERENCE_RADIUS_SCALE).max(0.1)
}

fn footprint_offset(camera_direction: Vec3, radius: f32) -> Vec3 {
    let direction = camera_direction.normalize_or_zero();
    Vec3::new(direction.x * radius, 0.0, direction.z * radius)
}

fn footprint_center(
    camera_position: Vec3,
    camera_direction: Vec3,
    rain_direction: Vec3,
    radius: f32,
) -> Vec3 {
    camera_position - rain_direction * RAIN_CULL_COP_OFFSET_METERS
        + footprint_offset(camera_direction, radius)
}

fn rain_up(rain_direction: Vec3) -> Vec3 {
    let right = if rain_direction.x.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::X
    };
    rain_direction.cross(right).normalize_or_zero()
}

fn light_space_bounds(camera: &CameraFrustumParams, far_meters: f32, view: Mat4) -> (Vec3, Vec3) {
    let mut min_ls = Vec3::splat(f32::MAX);
    let mut max_ls = Vec3::splat(f32::MIN);
    for corner in frustum_corners(camera, far_meters) {
        let point = view.transform_point3(corner);
        min_ls = min_ls.min(point);
        max_ls = max_ls.max(point);
    }
    (min_ls, max_ls)
}

fn light_space_depth_range(min_z: f32, max_z: f32) -> (f32, f32) {
    (
        -max_z - RAIN_ORTHO_XFORM_OFFSET_METERS,
        -min_z + 2.0 * RAIN_ORTHO_XFORM_OFFSET_METERS,
    )
}

fn frustum_corners(camera: &CameraFrustumParams, far_meters: f32) -> [Vec3; 8] {
    let transform = camera.camera_transform;
    let tan_y = (0.5 * camera.fov_y_radians).tan();
    let tan_x = tan_y * camera.aspect_ratio;
    let mut corners = [Vec3::ZERO; 8];
    for (index, corner) in corners.iter_mut().enumerate() {
        let far = index >= 4;
        let depth = if far { far_meters } else { camera.z_near };
        let x = if index & 1 == 0 { -1.0 } else { 1.0 };
        let y = if index & 2 == 0 { -1.0 } else { 1.0 };
        *corner = transform.translation
            + transform.forward() * depth
            + transform.right() * (x * depth * tan_x)
            + transform.up() * (y * depth * tan_y);
    }
    corners
}

fn snap_to_texel(combine: Mat4, smap_size: u32, pool_size: u32) -> Mat4 {
    let ndc = combine.project_point3(Vec3::ZERO);
    if !ndc.is_finite() {
        return combine;
    }
    let size = smap_size.max(1) as f32;
    let offset = 0.5 / pool_size.max(1) as f32;
    let pixel = Vec2::new(
        (ndc.x * 0.5 + 0.5) * size + offset,
        (-ndc.y * 0.5 + 0.5) * size + offset,
    )
    .floor();
    let snapped = Vec2::new(
        ((pixel.x - offset) / size) * 2.0 - 1.0,
        -(((pixel.y - offset) / size) * 2.0 - 1.0),
    );
    let adjustment = (snapped - ndc.truncate()).extend(0.0);
    Mat4::from_translation(adjustment) * combine
}

#[cfg(test)]
mod tests;

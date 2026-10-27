//! Spot and omnipart shadow transform and adaptive SMAP sizing (`Light_Render_Direct_ComputeXFS.cpp`).

use crate::{MoonlyLight, MoonlyLightType};
use bevy::prelude::*;

/// Tangent shift padding for split omnidirectional lights (`0.3f` in R4).
pub const TAN_SHIFT_OMNIPART: f32 = 0.3;

/// Tangent shift padding for point lights (`0.2007129f` in R4).
pub const TAN_SHIFT_POINT: f32 = 0.200_712_9;

/// Tangent shift padding for conical spot lights (`0.0610865f` in R4).
pub const TAN_SHIFT_SPOT: f32 = 0.061_086_5;

/// Minimum dynamic shadow map tile size in texels (`64` in R4).
pub const MIN_SPOT_SMAP_SIZE: u32 = 64;

/// Computed shadow matrices and adaptive SMAP resolution for a single spot/omnipart light.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct SpotShadowXform {
    /// Light-space view matrix (`X.S.view`).
    pub view: Mat4,
    /// Light-space perspective projection matrix (`X.S.project`).
    pub proj: Mat4,
    /// Combined `proj * view` shadow matrix (`X.S.combine`).
    pub combine: Mat4,
    /// Adaptive shadow map tile resolution in texels (`X.S.size`).
    pub smap_size: u32,
    /// Screen-space area metric in `[0.0, 1.0]`.
    pub screen_space_area: f32,
}

impl Default for SpotShadowXform {
    fn default() -> Self {
        Self {
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
            combine: Mat4::IDENTITY,
            smap_size: MIN_SPOT_SMAP_SIZE,
            screen_space_area: 0.0,
        }
    }
}

impl SpotShadowXform {
    /// Compute view/projection matrices and adaptive SMAP size (`CLight_Compute_XFORM_and_VIS::compute_xf_spot`).
    #[must_use]
    pub fn compute_xf_spot(
        light: &MoonlyLight,
        camera_pos: Vec3,
        max_pool_size: u32,
        prev_size: u32,
    ) -> Self {
        let l_dir = light.direction.normalize_or_zero();
        let fallback_right = if light.right.length_squared() > f32::EPSILON {
            Vec3::ZERO
        } else {
            let up_ref = if l_dir.y.abs() > 0.99 {
                Vec3::Z
            } else {
                Vec3::Y
            };
            up_ref.cross(l_dir).normalize_or_zero()
        };
        let l_right = if light.right.length_squared() > f32::EPSILON {
            light.right.normalize_or_zero()
        } else {
            fallback_right
        };
        let l_up = l_dir.cross(l_right).normalize_or_zero();
        let view = look_to_rh_unit_dir(light.position, l_dir, l_up);

        let dist_sq = camera_pos.distance_squared(light.position);
        let ssa = ((light.range * light.range) / (1.0 + dist_sq)).clamp(0.0, 1.0);
        let smap_size = if light.smap_size_override > 0 {
            u32::from(light.smap_size_override)
        } else {
            compute_adaptive_smap_size(light.color, ssa, max_pool_size, prev_size)
        };

        let tan_shift = match light.light_type {
            MoonlyLightType::Omnipart => TAN_SHIFT_OMNIPART,
            MoonlyLightType::Point | MoonlyLightType::Direct => TAN_SHIFT_POINT,
            MoonlyLightType::Spot => TAN_SHIFT_SPOT,
        };
        let min_fov = std::f32::consts::PI / 180.0;
        let max_fov = std::f32::consts::PI - 1.0e-3;
        let fov = (light.cone_angle + tan_shift).clamp(min_fov, max_fov);
        let z_far = light.range.max(0.05);
        let z_near = light.shadow_near_z.clamp(0.001, z_far);
        let proj = perspective_rh_mat4(fov, 1.0, z_near, z_far);
        let combine = proj * view;

        Self {
            view,
            proj,
            combine,
            smap_size,
            screen_space_area: ssa,
        }
    }
}

/// Build a right-handed view matrix looking along `dir` with `up` reference.
#[inline]
#[must_use]
pub fn look_to_rh_mat4(eye: Vec3, dir: Vec3, up: Vec3) -> Mat4 {
    look_to_rh_unit_dir(eye, dir.normalize_or_zero(), up)
}

#[inline]
#[must_use]
fn look_to_rh_unit_dir(eye: Vec3, f: Vec3, up: Vec3) -> Mat4 {
    let s = f.cross(up).normalize_or_zero();
    let u = s.cross(f);
    Mat4::from_cols(
        Vec4::new(s.x, u.x, -f.x, 0.0),
        Vec4::new(s.y, u.y, -f.y, 0.0),
        Vec4::new(s.z, u.z, -f.z, 0.0),
        Vec4::new(-eye.dot(s), -eye.dot(u), eye.dot(f), 1.0),
    )
}

/// Build a right-handed `[0, 1]` depth perspective projection matrix.
#[must_use]
pub fn perspective_rh_mat4(fov_y_radians: f32, aspect_ratio: f32, z_near: f32, z_far: f32) -> Mat4 {
    let h = 1.0 / (0.5 * fov_y_radians).tan();
    let w = h / aspect_ratio.max(1.0e-5);
    let r = z_far / (z_near - z_far);
    Mat4::from_cols(
        Vec4::new(w, 0.0, 0.0, 0.0),
        Vec4::new(0.0, h, 0.0, 0.0),
        Vec4::new(0.0, 0.0, r, -1.0),
        Vec4::new(0.0, 0.0, r * z_near, 0.0),
    )
}

/// Build a right-handed `[0, 1]` depth orthographic projection matrix.
#[must_use]
pub fn orthographic_rh_mat4(bounds: (f32, f32, f32, f32), near_far: (f32, f32)) -> Mat4 {
    let (left, right, bottom, top) = bounds;
    let (near, far) = near_far;
    let rcp_w = 1.0 / (right - left).max(1.0e-5);
    let rcp_h = 1.0 / (top - bottom).max(1.0e-5);
    let rcp_d = 1.0 / (near - far);
    Mat4::from_cols(
        Vec4::new(2.0 * rcp_w, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 * rcp_h, 0.0, 0.0),
        Vec4::new(0.0, 0.0, rcp_d, 0.0),
        Vec4::new(
            -(left + right) * rcp_w,
            -(top + bottom) * rcp_h,
            near * rcp_d,
            1.0,
        ),
    )
}

fn compute_adaptive_smap_size(color: Vec3, ssa: f32, max_pool_size: u32, prev_size: u32) -> u32 {
    let max_tile = (max_pool_size / 2).max(MIN_SPOT_SMAP_SIZE);
    let avg_rgb = (color.x + color.y + color.z) / 3.0;
    let luma_709 = 0.2125 * color.x + 0.7154 * color.y + 0.0721 * color.z;
    let intensity_factor = f32::midpoint(avg_rgb, luma_709).clamp(0.1, 2.0);
    let raw_size = (1.0 + ssa * (max_tile as f32 - 1.0)) * intensity_factor;
    let mut rounded = ((raw_size * 0.5).round() as u32).saturating_mul(2);
    rounded = rounded.clamp(MIN_SPOT_SMAP_SIZE, max_tile);

    if prev_size >= MIN_SPOT_SMAP_SIZE {
        let diff = (rounded as f32 - prev_size as f32).abs();
        if diff / (prev_size as f32) < 0.01 {
            return prev_size;
        }
    }
    rounded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_right_vector_uses_original_fallback_basis() {
        let light = MoonlyLight {
            position: Vec3::new(1.0, 2.0, 3.0),
            direction: Vec3::NEG_Y,
            right: Vec3::ZERO,
            ..MoonlyLight::default()
        };
        let xf = SpotShadowXform::compute_xf_spot(&light, Vec3::ZERO, 2048, 0);
        let expected = look_to_rh_mat4(light.position, Vec3::NEG_Y, Vec3::Z);
        assert!(xf.view.abs_diff_eq(expected, 1.0e-6));
        assert!(xf.view.determinant().abs() > 0.0);
    }

    #[test]
    fn shadow_near_plane_excludes_light_source_geometry() {
        let light = MoonlyLight {
            light_type: MoonlyLightType::Spot,
            shadow_near_z: 0.1,
            ..MoonlyLight::default()
        };
        let xf = SpotShadowXform::compute_xf_spot(&light, Vec3::ZERO, 2048, 0);
        let clip = xf.combine * Vec3::new(0.0, 2.95, 0.0).extend(1.0);
        assert!(clip.z < 0.0);
        assert!(xf.proj.z_axis.w < 0.0);
    }
}

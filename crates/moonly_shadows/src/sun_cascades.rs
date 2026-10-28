//! Cascaded sun shadow frustum and matrix calculation ported from `r2_R_sun.cpp`.

use bevy::prelude::*;
use moonly_core::{SunCascadeConfig, SunCascadeFootprint};

use crate::{cascade_planes::extract_frustum_planes, spot_xform::orthographic_rh_mat4};

/// Number of sun shadow cascades in `moonly-monolith` (`SE_SUN_NEAR`, `SE_SUN_MIDDLE`, `SE_SUN_FAR`).
pub const SUN_CASCADE_COUNT: usize = 3;

/// Index of the near sun cascade (`SE_SUN_NEAR = 0`).
pub const SE_SUN_NEAR: usize = 0;
/// Index of the middle sun cascade (`SE_SUN_MIDDLE = 1`).
pub const SE_SUN_MIDDLE: usize = 1;
/// Index of the far sun cascade (`SE_SUN_FAR = 2`).
pub const SE_SUN_FAR: usize = 2;

/// Camera parameters needed to compute sun cascade frustum slices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraFrustumParams {
    /// Camera world transform.
    pub camera_transform: Transform,
    /// Vertical field of view in radians.
    pub fov_y_radians: f32,
    /// Viewport width divided by height.
    pub aspect_ratio: f32,
    /// Near clip plane distance in meters.
    pub z_near: f32,
}

impl Default for CameraFrustumParams {
    fn default() -> Self {
        Self {
            camera_transform: Transform::from_xyz(0.0, 2.0, 8.0),
            fov_y_radians: std::f32::consts::FRAC_PI_3,
            aspect_ratio: 16.0 / 9.0,
            z_near: 0.1,
        }
    }
}

/// Computed matrices and silhouette planes for one sun shadow cascade (`Cascade` in `r2_R_sun.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct SunCascadeSlice {
    /// Light-space view matrix (`X.D.view`).
    pub view: Mat4,
    /// Light-space orthographic projection matrix (`X.D.project`).
    pub proj: Mat4,
    /// Combined `proj * view` shadow matrix (`X.D.combine`).
    pub combine: Mat4,
    /// Six world-space frustum culling planes (`(nx, ny, nz, d)`).
    pub cull_planes: [Vec4; 6],
    /// Near slice distance in meters.
    pub split_near: f32,
    /// Far slice distance in meters.
    pub split_far: f32,
    /// Depth bias derived from the active footprint policy.
    pub depth_bias: f32,
    /// Width of the visibility transition at the near split.
    pub blend_band: f32,
    /// Actual light-space width after the coverage fallback.
    pub footprint_size: f32,
}

impl Default for SunCascadeSlice {
    fn default() -> Self {
        Self {
            view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
            combine: Mat4::IDENTITY,
            cull_planes: [Vec4::ZERO; 6],
            split_near: 0.1,
            split_far: 20.0,
            depth_bias: 0.0,
            blend_band: 0.0,
            footprint_size: 20.0,
        }
    }
}

/// Resource storing all three computed sun cascades (`SE_SUN_NEAR`, `SE_SUN_MIDDLE`, `SE_SUN_FAR`).
#[derive(Resource, Debug, Clone, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct SunCascades {
    /// Three cascade slices ordered from near to far.
    pub cascades: [SunCascadeSlice; SUN_CASCADE_COUNT],
}

impl Default for SunCascades {
    fn default() -> Self {
        Self {
            cascades: [SunCascadeSlice::default(); SUN_CASCADE_COUNT],
        }
    }
}

#[derive(Clone, Copy)]
struct CascadeCameraBasis {
    pos: Vec3,
    fwd: Vec3,
    right: Vec3,
    up: Vec3,
    tan_half_x: f32,
    tan_half_y: f32,
    sun_f: Vec3,
    sun_s: Vec3,
    sun_u: Vec3,
}

impl CascadeCameraBasis {
    #[inline]
    fn new(cam: &CameraFrustumParams, sun_dir: Vec3) -> Self {
        let tan_half_y = (0.5 * cam.fov_y_radians).tan();
        let tan_half_x = tan_half_y * cam.aspect_ratio;
        let fwd = cam.camera_transform.forward().as_vec3();
        let right = cam.camera_transform.right().as_vec3();
        let up = cam.camera_transform.up().as_vec3();
        let pos = cam.camera_transform.translation;
        let sun_f = sun_dir.normalize_or_zero();
        let up_ref = if sun_f.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let sun_s = sun_f.cross(up_ref).normalize_or_zero();
        let sun_u = sun_s.cross(sun_f);
        Self {
            pos,
            fwd,
            right,
            up,
            tan_half_x,
            tan_half_y,
            sun_f,
            sun_s,
            sun_u,
        }
    }

    #[inline]
    fn view_matrix(&self, eye: Vec3) -> Mat4 {
        let s = self.sun_s;
        let u = self.sun_u;
        let f = self.sun_f;
        Mat4::from_cols(
            Vec4::new(s.x, u.x, -f.x, 0.0),
            Vec4::new(s.y, u.y, -f.y, 0.0),
            Vec4::new(s.z, u.z, -f.z, 0.0),
            Vec4::new(-eye.dot(s), -eye.dot(u), eye.dot(f), 1.0),
        )
    }
}

impl SunCascades {
    /// Recompute all three sun cascades from camera frustum parameters and `SunCascadeConfig`.
    pub fn compute(
        &mut self,
        cam: &CameraFrustumParams,
        sun_dir: Vec3,
        cfg: &SunCascadeConfig,
        smap_resolution: u32,
    ) {
        let basis = CascadeCameraBasis::new(cam, sun_dir);
        let splits = [
            (cam.z_near, cfg.near_split),
            (cfg.near_split, cfg.middle_split),
            (cfg.middle_split, cfg.far_split),
        ];
        for (idx, &(near_d, far_d)) in splits.iter().enumerate() {
            if let Some(slice) = self.cascades.get_mut(idx) {
                *slice = compute_single_cascade(&basis, (near_d, far_d), cfg, idx, smap_resolution);
            }
        }
    }
}

fn compute_single_cascade(
    basis: &CascadeCameraBasis,
    range: (f32, f32),
    cfg: &SunCascadeConfig,
    index: usize,
    smap_resolution: u32,
) -> SunCascadeSlice {
    let (split_near, split_far) = range;
    let corners = frustum_slice_corners(basis, split_near, split_far);
    let mut center = corners.iter().copied().sum::<Vec3>() * 0.125;
    let initial_view = basis.view_matrix(center - basis.sun_f * cfg.diagonal_offset);
    let (mut min_ls, mut max_ls) = light_space_aabb(&corners, initial_view);
    let fitted_size = (max_ls - min_ls).truncate().max_element().max(1.0);
    let footprint_size = match cfg.footprint {
        SunCascadeFootprint::FrustumFit => fitted_size,
        SunCascadeFootprint::ReferenceFixed => fitted_size.max(cfg.ortho_size(index)),
    };
    let world_per_texel = footprint_size / smap_resolution.max(1) as f32;
    if cfg.stabilize_texels {
        center = snap_center(center, basis, world_per_texel);
    }
    let view = basis.view_matrix(center - basis.sun_f * cfg.diagonal_offset);
    let padding = if cfg.stabilize_texels {
        world_per_texel
    } else {
        0.0
    };
    let bounds_center = (min_ls + max_ls) * 0.5;
    let half_size = footprint_size * 0.5 + padding;
    min_ls.x = bounds_center.x - half_size;
    max_ls.x = bounds_center.x + half_size;
    min_ls.y = bounds_center.y - half_size;
    max_ls.y = bounds_center.y + half_size;
    min_ls.z -= cfg.diagonal_offset;
    let proj = orthographic_rh_mat4(
        (min_ls.x, max_ls.x, min_ls.y, max_ls.y),
        (-max_ls.z, -min_ls.z),
    );
    let combine = proj * view;
    SunCascadeSlice {
        view,
        proj,
        combine,
        cull_planes: extract_frustum_planes(combine),
        split_near,
        split_far,
        depth_bias: cfg.cascade_depth_bias(footprint_size),
        blend_band: cfg.blend_band(split_far),
        footprint_size,
    }
}

fn snap_center(center: Vec3, basis: &CascadeCameraBasis, texel: f32) -> Vec3 {
    let x = (center.dot(basis.sun_s) / texel).floor() * texel;
    let y = (center.dot(basis.sun_u) / texel).floor() * texel;
    basis.sun_s * x + basis.sun_u * y + basis.sun_f * center.dot(basis.sun_f)
}

pub(crate) fn previous_cascade_combine(
    cascades: &[SunCascadeSlice; SUN_CASCADE_COUNT],
    index: usize,
) -> Mat4 {
    cascades
        .get(index.saturating_sub(1))
        .or_else(|| cascades.first())
        .map_or(Mat4::IDENTITY, |cascade| cascade.combine)
}

/// Return the current cascade weight across its near-split transition.
#[must_use]
pub fn cascade_blend_weight(view_depth: f32, split_near: f32, blend_band: f32) -> f32 {
    if !view_depth.is_finite() || !split_near.is_finite() || blend_band <= 0.0 {
        return f32::from(view_depth >= split_near);
    }
    ((view_depth - split_near) / blend_band).clamp(0.0, 1.0)
}

fn frustum_slice_corners(basis: &CascadeCameraBasis, near_d: f32, far_d: f32) -> [Vec3; 8] {
    let near_c = basis.pos + basis.fwd * near_d;
    let far_c = basis.pos + basis.fwd * far_d;
    let nh = near_d * basis.tan_half_y;
    let nw = near_d * basis.tan_half_x;
    let fh = far_d * basis.tan_half_y;
    let fw = far_d * basis.tan_half_x;
    let r_nw = basis.right * nw;
    let u_nh = basis.up * nh;
    let r_fw = basis.right * fw;
    let u_fh = basis.up * fh;
    [
        near_c - r_nw - u_nh,
        near_c + r_nw - u_nh,
        near_c + r_nw + u_nh,
        near_c - r_nw + u_nh,
        far_c - r_fw - u_fh,
        far_c + r_fw - u_fh,
        far_c + r_fw + u_fh,
        far_c - r_fw + u_fh,
    ]
}

fn light_space_aabb(corners: &[Vec3; 8], view: Mat4) -> (Vec3, Vec3) {
    let mut min_v = Vec3::splat(f32::MAX);
    let mut max_v = Vec3::splat(f32::MIN);
    for &corner in corners {
        let point = view.transform_point3(corner);
        min_v = min_v.min(point);
        max_v = max_v.max(point);
    }
    (min_v, max_v)
}

#[cfg(test)]
mod tests;

//! Cascaded sun shadow frustum and matrix calculation ported from `r2_R_sun.cpp`.

use bevy::prelude::*;
use moonly_core::SunCascadeConfig;

use crate::spot_xform::{look_to_rh_mat4, orthographic_rh_mat4};

/// Number of sun shadow cascades in `xray-monolith` (`SE_SUN_NEAR`, `SE_SUN_MIDDLE`, `SE_SUN_FAR`).
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

impl SunCascades {
    /// Recompute all three sun cascades from camera frustum parameters and `SunCascadeConfig`.
    pub fn compute(
        &mut self,
        cam: &CameraFrustumParams,
        sun_dir: Vec3,
        cfg: &SunCascadeConfig,
        smap_resolution: u32,
    ) {
        let splits = [
            (cam.z_near, cfg.near_split),
            (cfg.near_split, cfg.middle_split),
            (cfg.middle_split, cfg.far_split),
        ];
        for (idx, &(near_d, far_d)) in splits.iter().enumerate() {
            if let Some(slice) = self.cascades.get_mut(idx) {
                *slice =
                    compute_single_cascade(cam, sun_dir, (near_d, far_d), cfg, smap_resolution);
            }
        }
    }
}

fn compute_single_cascade(
    cam: &CameraFrustumParams,
    sun_dir: Vec3,
    range: (f32, f32),
    cfg: &SunCascadeConfig,
    smap_resolution: u32,
) -> SunCascadeSlice {
    let (split_near, split_far) = range;
    let corners = frustum_slice_corners(cam, split_near, split_far);
    let mut center = Vec3::ZERO;
    for c in &corners {
        center += *c;
    }
    center *= 0.125;

    let l_dir = sun_dir.normalize_or_zero();
    let up_ref = if l_dir.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let eye = center - l_dir * cfg.diagonal_offset;
    let view = look_to_rh_mat4(eye, l_dir, up_ref);

    let (mut min_ls, mut max_ls) = light_space_aabb(&corners, view);
    min_ls.z -= cfg.diagonal_offset;

    if cfg.stabilize_texels && smap_resolution > 0 {
        let span = (max_ls - min_ls).truncate().max_element().max(1.0);
        let world_per_texel = span / (smap_resolution as f32);
        min_ls.x = (min_ls.x / world_per_texel).floor() * world_per_texel;
        min_ls.y = (min_ls.y / world_per_texel).floor() * world_per_texel;
        max_ls.x = min_ls.x + span;
        max_ls.y = min_ls.y + span;
    }

    let proj = orthographic_rh_mat4(
        (min_ls.x, max_ls.x, min_ls.y, max_ls.y),
        (-max_ls.z, -min_ls.z),
    );
    let combine = proj * view;
    let cull_planes = extract_frustum_planes(combine);

    SunCascadeSlice {
        view,
        proj,
        combine,
        cull_planes,
        split_near,
        split_far,
    }
}

fn frustum_slice_corners(cam: &CameraFrustumParams, near_d: f32, far_d: f32) -> [Vec3; 8] {
    let tan_half_y = (0.5 * cam.fov_y_radians).tan();
    let tan_half_x = tan_half_y * cam.aspect_ratio;
    let fwd = cam.camera_transform.forward().as_vec3();
    let right = cam.camera_transform.right().as_vec3();
    let up = cam.camera_transform.up().as_vec3();
    let pos = cam.camera_transform.translation;

    let near_c = pos + fwd * near_d;
    let far_c = pos + fwd * far_d;
    let nh = near_d * tan_half_y;
    let nw = near_d * tan_half_x;
    let fh = far_d * tan_half_y;
    let fw = far_d * tan_half_x;

    [
        near_c - right * nw - up * nh,
        near_c + right * nw - up * nh,
        near_c + right * nw + up * nh,
        near_c - right * nw + up * nh,
        far_c - right * fw - up * fh,
        far_c + right * fw - up * fh,
        far_c + right * fw + up * fh,
        far_c - right * fw + up * fh,
    ]
}

fn light_space_aabb(corners: &[Vec3; 8], view: Mat4) -> (Vec3, Vec3) {
    let mut min_v = Vec3::splat(f32::MAX);
    let mut max_v = Vec3::splat(f32::MIN);
    for &c in corners {
        let ls = view.transform_point3(c);
        min_v = min_v.min(ls);
        max_v = max_v.max(ls);
    }
    (min_v, max_v)
}

fn extract_frustum_planes(m: Mat4) -> [Vec4; 6] {
    let r0 = m.row(0);
    let r1 = m.row(1);
    let r2 = m.row(2);
    let r3 = m.row(3);
    [
        normalize_plane(r3 + r0),
        normalize_plane(r3 - r0),
        normalize_plane(r3 + r1),
        normalize_plane(r3 - r1),
        normalize_plane(r3 + r2),
        normalize_plane(r3 - r2),
    ]
}

fn normalize_plane(p: Vec4) -> Vec4 {
    let len = p.truncate().length();
    if len > 1.0e-6 { p / len } else { p }
}

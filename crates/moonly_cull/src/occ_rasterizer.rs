//! Hierarchical 64x64 software occlusion rasterizer ported from `occRasterizer.h` and `occRasterizer.cpp`.

mod occ_query;

use bevy::prelude::*;

/// Level 0 raster resolution (`occ_dim_0 = 64`).
pub const OCC_DIM_0: usize = 64;
/// Level 1 pyramid resolution (`occ_dim_1 = 32`).
pub const OCC_DIM_1: usize = 32;
/// Level 2 pyramid resolution (`occ_dim_2 = 16`).
pub const OCC_DIM_2: usize = 16;
/// Level 3 pyramid resolution (`occ_dim_3 = 8`).
pub const OCC_DIM_3: usize = 8;

/// Fixed-point far plane depth constant (`df_2_s32(1.0)`).
pub const OCC_DEPTH_FAR: i32 = i32::MAX;

/// Convert a normalized `[0.0, 1.0]` depth value to signed 32-bit fixed point (`df_2_s32`).
#[inline]
#[must_use]
pub fn df_2_s32(depth: f32) -> i32 {
    let clamped = depth.clamp(0.0, 1.0);
    (f64::from(clamped) * 2_147_483_647.0) as i32
}

/// Screen-space rectangle and test depth for an occlusion query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OccTestRect {
    /// Minimum X coordinate in `[0, 63]`.
    pub x0: usize,
    /// Minimum Y coordinate in `[0, 63]`.
    pub y0: usize,
    /// Maximum X coordinate in `[0, 63]`.
    pub x1: usize,
    /// Maximum Y coordinate in `[0, 63]`.
    pub y1: usize,
    /// Normalized query minimum depth in `[0.0, 1.0]`.
    pub depth: f32,
}

/// Four-level 64x64 hierarchical Z-buffer ported from `occRasterizer` (`occRasterizer.h`).
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct OccRasterizer {
    /// Level 0 fixed-point depth buffer (`64x64`).
    pub buf_depth_0: Vec<i32>,
    /// Level 1 max-depth pyramid (`32x32`).
    pub buf_depth_1: Vec<i32>,
    /// Level 2 max-depth pyramid (`16x16`).
    pub buf_depth_2: Vec<i32>,
    /// Level 3 max-depth pyramid (`8x8`).
    pub buf_depth_3: Vec<i32>,
    /// Scratch buffer used during 3x3 cross dilation.
    scratch_0: Vec<i32>,
}

impl Default for OccRasterizer {
    fn default() -> Self {
        Self {
            buf_depth_0: vec![OCC_DEPTH_FAR; OCC_DIM_0 * OCC_DIM_0],
            buf_depth_1: vec![OCC_DEPTH_FAR; OCC_DIM_1 * OCC_DIM_1],
            buf_depth_2: vec![OCC_DEPTH_FAR; OCC_DIM_2 * OCC_DIM_2],
            buf_depth_3: vec![OCC_DEPTH_FAR; OCC_DIM_3 * OCC_DIM_3],
            scratch_0: vec![OCC_DEPTH_FAR; OCC_DIM_0 * OCC_DIM_0],
        }
    }
}

impl OccRasterizer {
    /// Reset all depth levels to the far plane (`occRasterizer::clear`).
    #[inline]
    pub fn clear(&mut self) {
        self.buf_depth_0.fill(OCC_DEPTH_FAR);
        self.buf_depth_1.fill(OCC_DEPTH_FAR);
        self.buf_depth_2.fill(OCC_DEPTH_FAR);
        self.buf_depth_3.fill(OCC_DEPTH_FAR);
    }

    /// Rasterize a screen-space triangle (`x, y` in `[0, 64]`, `z` in `[0, 1]`) into level 0.
    #[expect(
        unsafe_code,
        reason = "Coordinates are clamped to 0..64 and buffer length is verified before raw pointer access."
    )]
    pub fn rasterize_triangle(&mut self, v0: Vec3, v1: Vec3, v2: Vec3) {
        if self.buf_depth_0.len() < OCC_DIM_0 * OCC_DIM_0 {
            return;
        }
        let min_x = v0.x.min(v1.x).min(v2.x).floor().max(0.0) as usize;
        let max_x = v0.x.max(v1.x).max(v2.x).ceil().min(63.0) as usize;
        let min_y = v0.y.min(v1.y).min(v2.y).floor().max(0.0) as usize;
        let max_y = v0.y.max(v1.y).max(v2.y).ceil().min(63.0) as usize;
        if min_x > max_x || min_y > max_y {
            return;
        }
        let area = (v2.x - v0.x) * (v1.y - v0.y) - (v2.y - v0.y) * (v1.x - v0.x);
        if area.abs() < 1.0e-5 {
            return;
        }
        let inv_area = 1.0 / area;
        let dx12 = v2.x - v1.x;
        let dy12 = v2.y - v1.y;
        let dx20 = v0.x - v2.x;
        let dy20 = v0.y - v2.y;
        let is_flat_z = v0.z.to_bits() == v1.z.to_bits() && v1.z.to_bits() == v2.z.to_bits();
        let flat_z_val = if is_flat_z { df_2_s32(v0.z) } else { 0 };
        let dst_ptr = self.buf_depth_0.as_mut_ptr();

        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            let row_term0 = (py - v1.y) * dx12;
            let row_term1 = (py - v2.y) * dx20;
            let row_off = y * OCC_DIM_0;
            let mut hit_row = false;
            for x in min_x..=max_x {
                let px = x as f32 + 0.5;
                let w0 = ((px - v1.x) * dy12 - row_term0) * inv_area;
                if w0 < 0.0 {
                    if hit_row {
                        break;
                    }
                    continue;
                }
                let w1 = ((px - v2.x) * dy20 - row_term1) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w1 < 0.0 || w2 < 0.0 {
                    if hit_row {
                        break;
                    }
                    continue;
                }
                hit_row = true;
                let z = if is_flat_z {
                    flat_z_val
                } else {
                    df_2_s32(w0 * v0.z + w1 * v1.z + w2 * v2.z)
                };
                // SAFETY: `y <= 63`, `x <= 63`, and `buf_depth_0.len() >= 4096`.
                unsafe {
                    let cell = dst_ptr.add(row_off + x);
                    if z < *cell {
                        *cell = z;
                    }
                }
            }
        }
    }

    /// Apply 3x3 cross crack-filling dilation and build the 4-level Hi-Z pyramid (`propagade`).
    pub fn propagate(&mut self) {
        self.dilate_cross_level0();
        downsample_max_2x2(
            &self.buf_depth_0,
            OCC_DIM_0,
            &mut self.buf_depth_1,
            OCC_DIM_1,
        );
        downsample_max_2x2(
            &self.buf_depth_1,
            OCC_DIM_1,
            &mut self.buf_depth_2,
            OCC_DIM_2,
        );
        downsample_max_2x2(
            &self.buf_depth_2,
            OCC_DIM_2,
            &mut self.buf_depth_3,
            OCC_DIM_3,
        );
    }

    #[expect(
        unsafe_code,
        reason = "Interior coordinates 1..63 guarantee all 4-neighbor offsets are in 0..4096."
    )]
    fn dilate_cross_level0(&mut self) {
        if self.buf_depth_0.len() < OCC_DIM_0 * OCC_DIM_0
            || self.scratch_0.len() < OCC_DIM_0 * OCC_DIM_0
        {
            return;
        }
        self.scratch_0.copy_from_slice(&self.buf_depth_0);
        let src_ptr = self.scratch_0.as_ptr();
        let dst_ptr = self.buf_depth_0.as_mut_ptr();
        for y in 1..(OCC_DIM_0 - 1) {
            let row_off = y * OCC_DIM_0;
            for x in 1..(OCC_DIM_0 - 1) {
                let idx = row_off + x;
                // SAFETY: `1 <= y < 63` and `1 <= x < 63`, so `idx` and its 4 neighbors are `< 4096`.
                unsafe {
                    if *src_ptr.add(idx) < OCC_DEPTH_FAR {
                        continue;
                    }
                    let n2 = *src_ptr.add(idx - 1);
                    let n3 = *src_ptr.add(idx + 1);
                    let n0 = *src_ptr.add(idx - OCC_DIM_0);
                    if (n2 & n3 & n0) == OCC_DEPTH_FAR {
                        continue;
                    }
                    let n1 = *src_ptr.add(idx + OCC_DIM_0);
                    let (mut valid_count, mut max_d) = if n0 < OCC_DEPTH_FAR {
                        (1_u32, n0)
                    } else {
                        (0_u32, 0_i32)
                    };
                    if n1 < OCC_DEPTH_FAR {
                        valid_count += 1;
                        if n1 > max_d {
                            max_d = n1;
                        }
                    }
                    if n2 < OCC_DEPTH_FAR {
                        valid_count += 1;
                        if n2 > max_d {
                            max_d = n2;
                        }
                    }
                    if n3 < OCC_DEPTH_FAR {
                        valid_count += 1;
                        if n3 > max_d {
                            max_d = n3;
                        }
                    }
                    if valid_count >= 2 {
                        *dst_ptr.add(idx) = max_d;
                    }
                }
            }
        }
    }
}

#[expect(
    unsafe_code,
    reason = "Pyramid dimensions are verified before loop so 2x2 source and 1x1 destination indices are in-bounds."
)]
#[inline]
fn downsample_max_2x2(src: &[i32], src_dim: usize, dst: &mut [i32], dst_dim: usize) {
    if src.len() < src_dim * src_dim || dst.len() < dst_dim * dst_dim || src_dim < dst_dim * 2 {
        return;
    }
    let src_ptr = src.as_ptr();
    let dst_ptr = dst.as_mut_ptr();
    for y in 0..dst_dim {
        let sy0 = (y * 2) * src_dim;
        let sy1 = sy0 + src_dim;
        let dst_row = y * dst_dim;
        for x in 0..dst_dim {
            let sx = x * 2;
            // SAFETY: `y < dst_dim`, `x < dst_dim`, `src_dim >= 2 * dst_dim`, and slice lengths are checked above.
            unsafe {
                let d00 = *src_ptr.add(sy0 + sx);
                let d10 = *src_ptr.add(sy0 + sx + 1);
                let d01 = *src_ptr.add(sy1 + sx);
                let d11 = *src_ptr.add(sy1 + sx + 1);
                let m0 = if d00 > d10 { d00 } else { d10 };
                let m1 = if d01 > d11 { d01 } else { d11 };
                *dst_ptr.add(dst_row + x) = if m0 > m1 { m0 } else { m1 };
            }
        }
    }
}

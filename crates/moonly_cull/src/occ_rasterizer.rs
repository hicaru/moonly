//! Hierarchical 64x64 software occlusion rasterizer ported from `occRasterizer.h` and `occRasterizer.cpp`.

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
    pub fn clear(&mut self) {
        self.buf_depth_0.fill(OCC_DEPTH_FAR);
        self.buf_depth_1.fill(OCC_DEPTH_FAR);
        self.buf_depth_2.fill(OCC_DEPTH_FAR);
        self.buf_depth_3.fill(OCC_DEPTH_FAR);
    }

    /// Rasterize a screen-space triangle (`x, y` in `[0, 64]`, `z` in `[0, 1]`) into level 0.
    pub fn rasterize_triangle(&mut self, v0: Vec3, v1: Vec3, v2: Vec3) {
        let min_x = v0.x.min(v1.x).min(v2.x).floor().max(0.0) as usize;
        let max_x = v0.x.max(v1.x).max(v2.x).ceil().min(63.0) as usize;
        let min_y = v0.y.min(v1.y).min(v2.y).floor().max(0.0) as usize;
        let max_y = v0.y.max(v1.y).max(v2.y).ceil().min(63.0) as usize;
        let area = edge_fn(v0.truncate(), v1.truncate(), v2.truncate());
        if area.abs() < 1.0e-5 {
            return;
        }
        let inv_area = 1.0 / area;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = edge_fn(v1.truncate(), v2.truncate(), p) * inv_area;
                let w1 = edge_fn(v2.truncate(), v0.truncate(), p) * inv_area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let z = df_2_s32(w0 * v0.z + w1 * v1.z + w2 * v2.z);
                    if let Some(cell) = self.buf_depth_0.get_mut(y * OCC_DIM_0 + x)
                        && z < *cell
                    {
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

    fn dilate_cross_level0(&mut self) {
        self.scratch_0.copy_from_slice(&self.buf_depth_0);
        for y in 1..(OCC_DIM_0 - 1) {
            for x in 1..(OCC_DIM_0 - 1) {
                let idx = y * OCC_DIM_0 + x;
                let Some(&center) = self.scratch_0.get(idx) else {
                    continue;
                };
                if center < OCC_DEPTH_FAR {
                    continue;
                }
                if let Some(filled) = cross_fill_pixel(&self.scratch_0, x, y)
                    && let Some(dst) = self.buf_depth_0.get_mut(idx)
                {
                    *dst = filled;
                }
            }
        }
    }

    /// Faithful XRay-Monolith hierarchical occlusion test (`occRasterizer::test`).
    /// Returns `-1` when fully occluded and `1` when at least one sample is visible.
    #[must_use]
    pub fn test_monolith(&self, rect: OccTestRect) -> i32 {
        let depth_i32 = df_2_s32(rect.depth);
        let x0 = rect.x0.min(OCC_DIM_0 - 1);
        let y0 = rect.y0.min(OCC_DIM_0 - 1);
        let x1 = rect.x1.min(OCC_DIM_0 - 1).max(x0);
        let y1 = rect.y1.min(OCC_DIM_0 - 1).max(y0);
        for my in (y0 >> 3)..=(y1 >> 3) {
            for mx in (x0 >> 3)..=(x1 >> 3) {
                let res = self.test_cell_l3(mx, my, (x0, y0, x1, y1), depth_i32);
                if res > 0 {
                    return 1;
                }
            }
        }
        -1
    }

    fn test_cell_l3(
        &self,
        mx: usize,
        my: usize,
        bounds: (usize, usize, usize, usize),
        d: i32,
    ) -> i32 {
        let Some(&cell_d) = self.buf_depth_3.get(my * OCC_DIM_3 + mx) else {
            return 1;
        };
        if d > cell_d {
            return -1;
        }
        let (x0, y0, x1, y1) = bounds;
        let sx0 = (mx << 1).max(x0 >> 2);
        let sy0 = (my << 1).max(y0 >> 2);
        let sx1 = ((mx << 1) + 1).min(x1 >> 2);
        let sy1 = ((my << 1) + 1).min(y1 >> 2);
        for y in sy0..=sy1 {
            for x in sx0..=sx1 {
                if self.test_cell_l2(x, y, bounds, d) > 0 {
                    return 1;
                }
            }
        }
        -1
    }

    fn test_cell_l2(
        &self,
        mx: usize,
        my: usize,
        bounds: (usize, usize, usize, usize),
        d: i32,
    ) -> i32 {
        let Some(&cell_d) = self.buf_depth_2.get(my * OCC_DIM_2 + mx) else {
            return 1;
        };
        if d > cell_d {
            return -1;
        }
        let (x0, y0, x1, y1) = bounds;
        let sx0 = (mx << 1).max(x0 >> 1);
        let sy0 = (my << 1).max(y0 >> 1);
        let sx1 = ((mx << 1) + 1).min(x1 >> 1);
        let sy1 = ((my << 1) + 1).min(y1 >> 1);
        for y in sy0..=sy1 {
            for x in sx0..=sx1 {
                if self.test_cell_l1(x, y, bounds, d) > 0 {
                    return 1;
                }
            }
        }
        -1
    }

    fn test_cell_l1(
        &self,
        mx: usize,
        my: usize,
        bounds: (usize, usize, usize, usize),
        d: i32,
    ) -> i32 {
        let Some(&cell_d) = self.buf_depth_1.get(my * OCC_DIM_1 + mx) else {
            return 1;
        };
        if d > cell_d {
            return -1;
        }
        let (x0, y0, x1, y1) = bounds;
        let sx0 = (mx << 1).max(x0);
        let sy0 = (my << 1).max(y0);
        let sx1 = ((mx << 1) + 1).min(x1);
        let sy1 = ((my << 1) + 1).min(y1);
        for y in sy0..=sy1 {
            for x in sx0..=sx1 {
                let Some(&d0) = self.buf_depth_0.get(y * OCC_DIM_0 + x) else {
                    return 1;
                };
                if d <= d0 {
                    return 1;
                }
            }
        }
        -1
    }

    /// Enhanced Moonly flat-pyramid occlusion test with coarse early rejection and slice scans.
    #[must_use]
    pub fn test_enhanced_fast(&self, rect: OccTestRect) -> i32 {
        let d = df_2_s32(rect.depth);
        let x0 = rect.x0.min(OCC_DIM_0 - 1);
        let y0 = rect.y0.min(OCC_DIM_0 - 1);
        let x1 = rect.x1.min(OCC_DIM_0 - 1).max(x0);
        let y1 = rect.y1.min(OCC_DIM_0 - 1).max(y0);
        if !any_cell_visible(
            &self.buf_depth_3,
            OCC_DIM_3,
            (x0 >> 3, y0 >> 3, x1 >> 3, y1 >> 3),
            d,
        ) {
            return -1;
        }
        if !any_cell_visible(
            &self.buf_depth_1,
            OCC_DIM_1,
            (x0 >> 1, y0 >> 1, x1 >> 1, y1 >> 1),
            d,
        ) {
            return -1;
        }
        if any_cell_visible(&self.buf_depth_0, OCC_DIM_0, (x0, y0, x1, y1), d) {
            1
        } else {
            -1
        }
    }
}

fn edge_fn(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)
}

fn cross_fill_pixel(src: &[i32], x: usize, y: usize) -> Option<i32> {
    let neighbors = [
        src.get((y - 1) * OCC_DIM_0 + x)
            .copied()
            .unwrap_or(OCC_DEPTH_FAR),
        src.get((y + 1) * OCC_DIM_0 + x)
            .copied()
            .unwrap_or(OCC_DEPTH_FAR),
        src.get(y * OCC_DIM_0 + (x - 1))
            .copied()
            .unwrap_or(OCC_DEPTH_FAR),
        src.get(y * OCC_DIM_0 + (x + 1))
            .copied()
            .unwrap_or(OCC_DEPTH_FAR),
    ];
    let mut valid_count = 0_u32;
    let mut max_d = 0_i32;
    for n in neighbors {
        if n < OCC_DEPTH_FAR {
            valid_count += 1;
            max_d = max_d.max(n);
        }
    }
    (valid_count >= 2).then_some(max_d)
}

fn downsample_max_2x2(src: &[i32], src_dim: usize, dst: &mut [i32], dst_dim: usize) {
    for y in 0..dst_dim {
        let sy = y * 2;
        for x in 0..dst_dim {
            let sx = x * 2;
            let d00 = src.get(sy * src_dim + sx).copied().unwrap_or(OCC_DEPTH_FAR);
            let d10 = src
                .get(sy * src_dim + sx + 1)
                .copied()
                .unwrap_or(OCC_DEPTH_FAR);
            let d01 = src
                .get((sy + 1) * src_dim + sx)
                .copied()
                .unwrap_or(OCC_DEPTH_FAR);
            let d11 = src
                .get((sy + 1) * src_dim + sx + 1)
                .copied()
                .unwrap_or(OCC_DEPTH_FAR);
            if let Some(out) = dst.get_mut(y * dst_dim + x) {
                *out = d00.max(d10).max(d01).max(d11);
            }
        }
    }
}

fn any_cell_visible(buf: &[i32], dim: usize, bounds: (usize, usize, usize, usize), d: i32) -> bool {
    let (x0, y0, x1, y1) = bounds;
    for y in y0..=y1 {
        let row_start = y * dim + x0;
        let row_end = y * dim + x1 + 1;
        let Some(row) = buf.get(row_start..row_end) else {
            continue;
        };
        if row.iter().any(|&cell| d <= cell) {
            return true;
        }
    }
    false
}

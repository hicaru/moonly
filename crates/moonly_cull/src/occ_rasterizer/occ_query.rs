use super::{OCC_DIM_0, OCC_DIM_1, OCC_DIM_2, OCC_DIM_3, OccRasterizer, OccTestRect, df_2_s32};

impl OccRasterizer {
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
                if d < d0 {
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

fn any_cell_visible(buf: &[i32], dim: usize, bounds: (usize, usize, usize, usize), d: i32) -> bool {
    let (x0, y0, x1, y1) = bounds;
    for y in y0..=y1 {
        let row_start = y * dim + x0;
        let row_end = y * dim + x1 + 1;
        let Some(row) = buf.get(row_start..row_end) else {
            continue;
        };
        if row.iter().any(|&cell| d < cell) {
            return true;
        }
    }
    false
}

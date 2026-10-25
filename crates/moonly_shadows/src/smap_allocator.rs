//! Guillotine 2D shadow map atlas allocator ported from `SMAP_Allocator.h`.

use bevy::prelude::*;

/// Inclusive 2D integer rectangle matching `Irect` in `SMAP_Allocator.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub struct SmapRect {
    /// Minimum inclusive X coordinate (`a.x`).
    pub min_x: u32,
    /// Minimum inclusive Y coordinate (`a.y`).
    pub min_y: u32,
    /// Maximum inclusive X coordinate (`b.x`).
    pub max_x: u32,
    /// Maximum inclusive Y coordinate (`b.y`).
    pub max_y: u32,
}

impl SmapRect {
    /// Construct a rectangle from inclusive bounds `(min_x, min_y)` to `(max_x, max_y)`.
    #[inline]
    #[must_use]
    pub const fn new(min_x: u32, min_y: u32, max_x: u32, max_y: u32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    /// Return the horizontal span (`b.x - a.x + 1`).
    #[inline]
    #[must_use]
    pub const fn width(&self) -> u32 {
        if self.max_x >= self.min_x {
            self.max_x - self.min_x + 1
        } else {
            0
        }
    }

    /// Return the vertical span (`b.y - a.y + 1`).
    #[inline]
    #[must_use]
    pub const fn height(&self) -> u32 {
        if self.max_y >= self.min_y {
            self.max_y - self.min_y + 1
        } else {
            0
        }
    }

    /// Check whether a square of `size` texels fits inside this free rectangle.
    #[inline]
    #[must_use]
    pub const fn fits_square(&self, size: u32) -> bool {
        self.width() >= size && self.height() >= size
    }

    /// Compute the 2-texel inset shadow atlas transform matrix (`m_TexelAdjust` in R4).
    #[must_use]
    pub fn texel_adjust_matrix(&self, square_size: u32, pool_size: u32, bias: f32) -> Mat4 {
        let smap = (pool_size.max(1)) as f32;
        let inner = square_size.saturating_sub(2) as f32;
        let view_dim = inner / smap;
        let view_sx = (self.min_x + 1) as f32 / smap;
        let view_sy = (self.min_y + 1) as f32 / smap;
        let half_dim = 0.5 * view_dim;
        let texel_offset = 0.5 / smap;
        Mat4::from_cols(
            Vec4::new(half_dim, 0.0, 0.0, 0.0),
            Vec4::new(0.0, -half_dim, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(
                half_dim + view_sx + texel_offset,
                half_dim + view_sy + texel_offset,
                bias,
                1.0,
            ),
        )
    }
}

/// Shadow map atlas allocator ported from `SMAP_Allocator` (`SMAP_Allocator.h`).
#[derive(Resource, Debug, Clone, Default, Reflect)]
#[reflect(Resource)]
pub struct SmapAllocator {
    /// Total atlas edge dimension in texels (`psize`).
    pub pool_size: u32,
    /// Free rectangles used by the enhanced BSSF pass.
    pub free_rects: Vec<SmapRect>,
    /// Allocated rectangles during the current pass (`stack`).
    pub allocated: Vec<SmapRect>,
    /// Failed allocation requests during the current pass.
    pub failed_allocations: usize,
    critical_points: Vec<IVec2>,
}

impl SmapAllocator {
    /// Create and initialize an allocator with `pool_size` texels (`SMAP_Allocator::initialize`).
    #[inline]
    #[must_use]
    pub fn new(pool_size: u32) -> Self {
        let mut alloc = Self {
            pool_size: 0,
            free_rects: Vec::with_capacity(64),
            allocated: Vec::with_capacity(64),
            failed_allocations: 0,
            critical_points: Vec::with_capacity(64),
        };
        alloc.initialize(pool_size);
        alloc
    }

    /// Reset the atlas to a single free square of `pool_size` texels (`initialize`).
    #[inline]
    pub fn initialize(&mut self, pool_size: u32) {
        self.pool_size = pool_size;
        self.free_rects.clear();
        self.allocated.clear();
        self.critical_points.clear();
        self.failed_allocations = 0;
        if pool_size > 0 {
            self.free_rects
                .push(SmapRect::new(0, 0, pool_size - 1, pool_size - 1));
        }
    }

    /// Reset atlas allocations for a new frame.
    #[inline]
    pub fn begin_frame(&mut self, pool_size: u32) {
        self.initialize(pool_size);
    }

    /// Faithful critical-point insertion-order first-fit allocation (`SMAP_Allocator::push`).
    #[expect(
        unsafe_code,
        reason = "Pointer loops are bounded by `critical_points.len()` and `allocated.len()`."
    )]
    #[inline]
    pub fn push_monolith(&mut self, size: u32) -> Option<SmapRect> {
        if size <= 4 || size > self.pool_size {
            return self.allocation_failed();
        }
        let alloc_len = self.allocated.len();
        if alloc_len == 0 {
            let rect = SmapRect::new(0, 0, size - 1, size - 1);
            self.add_monolith_rect(rect);
            return Some(rect);
        }
        let cp_len = self.critical_points.len();
        let cp_ptr = self.critical_points.as_ptr();
        let alloc_ptr = self.allocated.as_ptr();
        let pool = self.pool_size;
        let mut point_index = 0_usize;
        while point_index < cp_len {
            // SAFETY: `point_index < cp_len`.
            let point = unsafe { *cp_ptr.add(point_index) };
            let px = point.x as u32;
            let py = point.y as u32;
            let max_x = px + size - 1;
            let max_y = py + size - 1;
            if max_x < pool && max_y < pool {
                let mut overlaps = false;
                let mut j = 0_usize;
                while j < alloc_len {
                    // SAFETY: `j < alloc_len`.
                    let other = unsafe { *alloc_ptr.add(j) };
                    if max_x >= other.min_x
                        && max_y >= other.min_y
                        && px <= other.max_x
                        && py <= other.max_y
                    {
                        overlaps = true;
                        break;
                    }
                    j += 1;
                }
                if !overlaps {
                    self.critical_points.remove(point_index);
                    let rect = SmapRect::new(px, py, max_x, max_y);
                    self.add_monolith_rect(rect);
                    return Some(rect);
                }
            }
            point_index += 1;
        }
        self.allocation_failed()
    }

    /// Enhanced Moonly Best-Short-Side-Fit (`BSSF`) allocation that minimizes fragment waste.
    #[expect(unsafe_code, reason = "Pointer loop is bounded by `free_rects.len()`.")]
    #[inline]
    pub fn push_enhanced_bssf(&mut self, size: u32) -> Option<SmapRect> {
        if size == 0 || size > self.pool_size {
            return self.allocation_failed();
        }
        let free_len = self.free_rects.len();
        let free_ptr = self.free_rects.as_ptr();
        let mut best_idx = usize::MAX;
        let mut best_score = u64::MAX;
        let mut idx = 0_usize;
        while idx < free_len {
            // SAFETY: `idx < free_len`.
            let rect = unsafe { *free_ptr.add(idx) };
            let w = rect.max_x - rect.min_x + 1;
            let h = rect.max_y - rect.min_y + 1;
            if w >= size && h >= size {
                let rem_w = w - size;
                let rem_h = h - size;
                let (short_side, long_side) = if rem_w <= rem_h {
                    (rem_w, rem_h)
                } else {
                    (rem_h, rem_w)
                };
                let score = (u64::from(short_side) << 32) | u64::from(long_side);
                if score == 0 {
                    return Some(self.allocate_at_index(idx, size));
                }
                if score < best_score {
                    best_score = score;
                    best_idx = idx;
                }
            }
            idx += 1;
        }
        if best_idx == usize::MAX {
            return self.allocation_failed();
        }
        Some(self.allocate_at_index(best_idx, size))
    }

    #[inline]
    fn add_monolith_rect(&mut self, rect: SmapRect) {
        self.allocated.push(rect);
        let right = IVec2::new((rect.max_x + 1) as i32, rect.min_y as i32);
        let down = IVec2::new(rect.min_x as i32, (rect.max_y + 1) as i32);
        let limit = self.pool_size as i32;
        if right.x < limit && right.y < limit {
            self.critical_points.push(right);
        }
        if down.x < limit && down.y < limit {
            self.critical_points.push(down);
        }
    }

    #[inline]
    const fn allocation_failed(&mut self) -> Option<SmapRect> {
        self.failed_allocations += 1;
        None
    }

    #[inline]
    fn allocate_at_index(&mut self, index: usize, size: u32) -> SmapRect {
        let free = self.free_rects.swap_remove(index);
        let result = SmapRect::new(
            free.min_x,
            free.min_y,
            free.min_x + size - 1,
            free.min_y + size - 1,
        );
        self.split_guillotine(free, size);
        self.allocated.push(result);
        result
    }

    #[inline]
    fn split_guillotine(&mut self, free: SmapRect, size: u32) {
        let bottom_min_y = free.min_y + size;
        if bottom_min_y <= free.max_y {
            self.free_rects.push(SmapRect::new(
                free.min_x,
                bottom_min_y,
                free.min_x + size - 1,
                free.max_y,
            ));
        }
        let right_min_x = free.min_x + size;
        if right_min_x <= free.max_x {
            self.free_rects.push(SmapRect::new(
                right_min_x,
                free.min_y,
                free.max_x,
                free.max_y,
            ));
        }
    }
}

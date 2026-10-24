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
    #[must_use]
    pub const fn width(&self) -> u32 {
        if self.max_x >= self.min_x {
            self.max_x - self.min_x + 1
        } else {
            0
        }
    }

    /// Return the vertical span (`b.y - a.y + 1`).
    #[must_use]
    pub const fn height(&self) -> u32 {
        if self.max_y >= self.min_y {
            self.max_y - self.min_y + 1
        } else {
            0
        }
    }

    /// Check whether a square of `size` texels fits inside this free rectangle.
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
        Mat4::from_cols(
            Vec4::new(half_dim, 0.0, 0.0, 0.0),
            Vec4::new(0.0, -half_dim, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(half_dim + view_sx, half_dim + view_sy, bias, 1.0),
        )
    }
}

/// Shadow map atlas allocator ported from `CSMAP_Allocator` (`SMAP_Allocator.h`).
#[derive(Resource, Debug, Clone, Default, Reflect)]
#[reflect(Resource)]
pub struct SmapAllocator {
    /// Total atlas edge dimension in texels (`psize`).
    pub pool_size: u32,
    /// Active free rectangles (`stack`).
    pub free_rects: Vec<SmapRect>,
    /// Allocated rectangles during the current pass.
    pub allocated: Vec<SmapRect>,
}

impl SmapAllocator {
    /// Create and initialize an allocator with `pool_size` texels (`CSMAP_Allocator::initialize`).
    #[must_use]
    pub fn new(pool_size: u32) -> Self {
        let mut alloc = Self::default();
        alloc.initialize(pool_size);
        alloc
    }

    /// Reset the atlas to a single free square of `pool_size` texels (`initialize`).
    pub fn initialize(&mut self, pool_size: u32) {
        let psize = pool_size.max(64);
        self.pool_size = psize;
        self.free_rects.clear();
        self.allocated.clear();
        self.free_rects
            .push(SmapRect::new(0, 0, psize - 1, psize - 1));
    }

    /// Faithful XRay-Monolith LIFO Guillotine allocation (`CSMAP_Allocator::push`).
    pub fn push_monolith(&mut self, size: u32) -> Option<SmapRect> {
        if size == 0 || size > self.pool_size {
            return None;
        }
        let found_idx = (0..self.free_rects.len()).rev().find(|&idx| {
            self.free_rects
                .get(idx)
                .is_some_and(|r| r.fits_square(size))
        })?;
        Some(self.allocate_at_index(found_idx, size))
    }

    /// Enhanced Moonly Best-Short-Side-Fit (`BSSF`) allocation that minimizes fragment waste.
    pub fn push_enhanced_bssf(&mut self, size: u32) -> Option<SmapRect> {
        if size == 0 || size > self.pool_size {
            return None;
        }
        let mut best_idx = None;
        let mut best_short_side = u32::MAX;
        let mut best_long_side = u32::MAX;
        for (idx, rect) in self.free_rects.iter().enumerate() {
            if !rect.fits_square(size) {
                continue;
            }
            let rem_w = rect.width() - size;
            let rem_h = rect.height() - size;
            let short_side = rem_w.min(rem_h);
            let long_side = rem_w.max(rem_h);
            if short_side < best_short_side
                || (short_side == best_short_side && long_side < best_long_side)
            {
                best_short_side = short_side;
                best_long_side = long_side;
                best_idx = Some(idx);
            }
        }
        Some(self.allocate_at_index(best_idx?, size))
    }

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

    fn split_guillotine(&mut self, free: SmapRect, size: u32) {
        let right_min_x = free.min_x + size;
        if right_min_x <= free.max_x {
            self.free_rects.push(SmapRect::new(
                right_min_x,
                free.min_y,
                free.max_x,
                free.max_y,
            ));
        }
        let bottom_min_y = free.min_y + size;
        if bottom_min_y <= free.max_y {
            self.free_rects.push(SmapRect::new(
                free.min_x,
                bottom_min_y,
                free.min_x + size - 1,
                free.max_y,
            ));
        }
    }
}

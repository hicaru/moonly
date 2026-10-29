use bevy::prelude::*;

pub(crate) const TRI_SKIP_MIN_FRAMES: u32 = 3;
pub(crate) const TRI_SKIP_MAX_EXCLUSIVE: u32 = 10;
pub(crate) const VIS_DELAY_MIN_FRAMES: u32 = 10;
pub(crate) const VIS_DELAY_MAX_EXCLUSIVE: u32 = 25;
pub(crate) const HOM_HIDDEN_RETEST_FRAMES: u32 = 1;
const HOM_JITTER_SEED: u64 = 0x484f_4d5f_4d4f_4f4e;

/// Enables deterministic temporal HOM caches for diagnostics and comparison.
#[derive(Resource, Debug, Clone, Copy, Reflect)]
#[reflect(Resource)]
pub struct MoonlyHomCacheConfig {
    /// Reuse triangle rejection and visibility results between updates.
    pub enabled: bool,
}

impl Default for MoonlyHomCacheConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Resource, Debug, Default)]
pub(crate) struct HomFrameClock(pub(crate) u32);

impl HomFrameClock {
    pub(crate) const fn advance(&mut self) -> u32 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
struct VisibilityEntry {
    key: u64,
    next_test_frame: u32,
    last_tested_frame: u32,
    visible: bool,
}

#[derive(Resource, Debug, Default)]
pub(crate) struct HomVisibilityCache {
    frame: u32,
    entries: Vec<VisibilityEntry>,
    pub(crate) reused: u32,
    pub(crate) tested: u32,
}

impl HomVisibilityCache {
    pub(crate) const fn set_frame(&mut self, frame: u32) {
        self.frame = frame;
        self.reused = 0;
        self.tested = 0;
    }

    pub(crate) fn latest_test_frame(&self) -> Option<u32> {
        self.entries
            .iter()
            .map(|entry| entry.last_tested_frame)
            .max()
    }

    pub(crate) fn should_test(&mut self, key: u64) -> Option<bool> {
        let Ok(index) = self.entries.binary_search_by_key(&key, |entry| entry.key) else {
            return None;
        };
        let entry = self.entries.get(index)?;
        if self.frame < entry.next_test_frame {
            self.reused = self.reused.saturating_add(1);
            return Some(entry.visible);
        }
        None
    }

    pub(crate) fn record(&mut self, key: u64, visible: bool) {
        self.tested = self.tested.saturating_add(1);
        let delay = if visible {
            jitter(
                key,
                self.frame,
                VIS_DELAY_MIN_FRAMES,
                VIS_DELAY_MAX_EXCLUSIVE,
            )
        } else {
            HOM_HIDDEN_RETEST_FRAMES
        };
        let entry = VisibilityEntry {
            key,
            next_test_frame: self.frame.wrapping_add(delay),
            last_tested_frame: self.frame,
            visible,
        };
        match self.entries.binary_search_by_key(&key, |entry| entry.key) {
            Ok(index) => {
                if let Some(slot) = self.entries.get_mut(index) {
                    *slot = entry;
                }
            }
            Err(index) => self.entries.insert(index, entry),
        }
    }

    #[cfg(test)]
    fn next_test_frame(&self, key: u64) -> Option<u32> {
        self.entries
            .binary_search_by_key(&key, |entry| entry.key)
            .ok()
            .and_then(|index| self.entries.get(index))
            .map(|entry| entry.next_test_frame)
    }

    #[cfg(test)]
    fn last_tested_frame(&self, key: u64) -> Option<u32> {
        self.entries
            .binary_search_by_key(&key, |entry| entry.key)
            .ok()
            .and_then(|index| self.entries.get(index))
            .map(|entry| entry.last_tested_frame)
    }
}

#[derive(Resource, Debug, Default)]
pub(crate) struct HomTriangleSkipCache {
    revision: u64,
    initialized: bool,
    skip: Vec<u32>,
    pub(crate) rasterized: u32,
    pub(crate) rejected: u32,
    pub(crate) skipped: u32,
}

impl HomTriangleSkipCache {
    pub(crate) fn sync_revision(&mut self, revision: u64, triangle_count: usize) {
        if self.initialized && self.revision == revision && self.skip.len() == triangle_count {
            return;
        }
        self.revision = revision;
        self.initialized = true;
        self.skip.clear();
        self.skip.resize(triangle_count, 0);
    }

    pub(crate) const fn begin_frame(&mut self) {
        self.rasterized = 0;
        self.rejected = 0;
        self.skipped = 0;
    }

    pub(crate) fn should_skip(&mut self, index: usize, frame: u32) -> bool {
        let skip = self.skip.get(index).copied().unwrap_or(0);
        let skip = skip > frame;
        if skip {
            self.skipped = self.skipped.saturating_add(1);
        }
        skip
    }

    pub(crate) fn record(&mut self, index: usize, frame: u32, rasterized: bool) {
        if rasterized {
            self.rasterized = self.rasterized.saturating_add(1);
            return;
        }
        self.rejected = self.rejected.saturating_add(1);
        let delay = jitter(
            index as u64,
            frame,
            TRI_SKIP_MIN_FRAMES,
            TRI_SKIP_MAX_EXCLUSIVE,
        );
        if let Some(slot) = self.skip.get_mut(index) {
            *slot = frame.wrapping_add(delay);
        }
    }

    #[cfg(test)]
    fn skip_frame(&self, index: usize) -> Option<u32> {
        self.skip.get(index).copied()
    }
}

/// Per-frame HOM work counters for performance validation.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct MoonlyHomDiagnostics {
    /// Completed HOM update time in microseconds.
    pub update_micros: u128,
    /// Rasterized triangles in the latest update.
    pub rasterized: u32,
    /// Rejected triangles in the latest update.
    pub rejected: u32,
    /// Previously rejected triangles skipped in the latest update.
    pub skipped: u32,
    /// Visibility results reused in the latest update.
    pub visibility_reused: u32,
    /// Visibility results tested in the latest update.
    pub visibility_tested: u32,
    /// Latest frame with a real visibility test.
    pub last_visibility_test_frame: Option<u32>,
}

fn jitter(key: u64, frame: u32, min: u32, max_exclusive: u32) -> u32 {
    let mut value = key ^ u64::from(frame) ^ HOM_JITTER_SEED;
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    min + (value as u32 % max_exclusive.saturating_sub(min).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_and_hidden_entries_use_reference_delays() {
        let mut cache = HomVisibilityCache::default();
        cache.set_frame(0);
        assert_eq!(cache.should_test(7), None);
        cache.record(7, true);
        let next = cache.next_test_frame(7).unwrap_or(0);
        assert!((10..25).contains(&next));
        assert_eq!(cache.last_tested_frame(7), Some(0));
        cache.set_frame(next - 1);
        assert_eq!(cache.should_test(7), Some(true));
        cache.set_frame(next);
        assert_eq!(cache.should_test(7), None);
        cache.record(7, false);
        assert_eq!(cache.next_test_frame(7), Some(next + 1));
        cache.set_frame(next + 1);
        assert_eq!(cache.should_test(7), None);
    }

    #[test]
    fn jitter_and_triangle_schedules_are_deterministic_and_bounded() {
        for frame in 0..50 {
            let delay = jitter(42, frame, 10, 25);
            assert_eq!(delay, jitter(42, frame, 10, 25));
            assert!((10..25).contains(&delay));
        }
        let mut cache = HomTriangleSkipCache::default();
        cache.sync_revision(1, 2);
        cache.record(0, 0, false);
        let skip = cache.skip_frame(0).unwrap_or(0);
        assert!((3..10).contains(&skip));
        assert!(cache.should_skip(0, skip - 1));
        assert!(!cache.should_skip(0, skip));
        cache.sync_revision(2, 2);
        assert_eq!(cache.skip_frame(0), Some(0));
    }
}

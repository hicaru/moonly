//! Shadow map caster occlusion state machine ported from `light_smapvis.h` and `light_smapvis.cpp`.

use bevy::prelude::*;

/// Sleep interval in frames between caster occlusion sweeps (`ps_r2_smapvis`).
pub const SMAP_VIS_SLEEP_FRAMES: u32 = 6;

/// State machine phases from `smapvis::state` in `light_smapvis.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum SmapVisPhase {
    /// Collect candidate shadow casters for the light frustum (`state_scan`).
    #[default]
    Scan,
    /// Wait a fixed frame window before re-probing casters (`state_sleep`).
    Sleep,
    /// Issue a single caster occlusion test per frame (`state_test`).
    Test,
    /// Clear the invisible list and restart the cycle (`state_reset`).
    Reset,
}

/// Per-light shadow caster occlusion tracker ported from `smapvis` (`light_smapvis.h`).
#[derive(Component, Debug, Clone, Default, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct SmapVisTracker {
    /// Current state machine phase.
    pub phase: SmapVisPhase,
    /// Candidate shadow caster entity IDs gathered during `Scan`.
    pub candidates: Vec<u32>,
    /// Casters confirmed occluded (`invisible` in `smapvis`).
    pub invisible: Vec<u32>,
    /// Frame counter when the tracker wakes from `Sleep`.
    pub sleep_until_frame: u32,
    /// Index of the candidate currently under test (`testQ_id`).
    pub current_test_index: usize,
    /// Result of the most recent occlusion probe (`0` = occluded, `> 0` = visible).
    pub last_probe_fragments: u32,
}

impl SmapVisTracker {
    /// Reset all cached caster lists and return to `Scan` (`smapvis::resetoccq`).
    pub fn reset(&mut self) {
        self.phase = SmapVisPhase::Scan;
        self.candidates.clear();
        self.invisible.clear();
        self.current_test_index = 0;
        self.last_probe_fragments = 1;
    }

    /// Check whether a shadow caster ID is currently marked invisible.
    #[must_use]
    pub fn is_caster_invisible(&self, caster_id: u32) -> bool {
        self.invisible.binary_search(&caster_id).is_ok()
    }

    /// Advance the `smapvis` state machine for `current_frame` (`smapvis::end` / `flushoccq`).
    pub fn step_frame(&mut self, current_frame: u32) {
        match self.phase {
            SmapVisPhase::Scan => {
                self.sleep_until_frame = current_frame.saturating_add(SMAP_VIS_SLEEP_FRAMES);
                self.current_test_index = 0;
                self.phase = SmapVisPhase::Sleep;
            }
            SmapVisPhase::Sleep => {
                if current_frame >= self.sleep_until_frame {
                    self.phase = if self.candidates.is_empty() {
                        SmapVisPhase::Reset
                    } else {
                        SmapVisPhase::Test
                    };
                }
            }
            SmapVisPhase::Test => {
                if self.last_probe_fragments == 0
                    && let Some(&caster_id) = self.candidates.get(self.current_test_index)
                    && let Err(pos) = self.invisible.binary_search(&caster_id)
                {
                    self.invisible.insert(pos, caster_id);
                }
                self.current_test_index += 1;
                if self.current_test_index >= self.candidates.len() {
                    self.sleep_until_frame = current_frame.saturating_add(SMAP_VIS_SLEEP_FRAMES);
                    self.phase = SmapVisPhase::Reset;
                } else {
                    self.sleep_until_frame = current_frame.saturating_add(1);
                    self.phase = SmapVisPhase::Sleep;
                }
            }
            SmapVisPhase::Reset => {
                if current_frame >= self.sleep_until_frame {
                    self.reset();
                }
            }
        }
    }
}

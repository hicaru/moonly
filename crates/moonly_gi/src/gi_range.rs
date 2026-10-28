pub(crate) const REFERENCE_MIN_ENERGY: f32 = 1.0 / 255.0;
const REFERENCE_MIN_RANGE: f32 = 0.1;

pub(crate) fn reference_range(reflected_energy: f32, energy_clip: f32) -> Option<f32> {
    if !reflected_energy.is_finite() || !energy_clip.is_finite() || reflected_energy < energy_clip {
        return None;
    }
    let range = (reflected_energy - REFERENCE_MIN_ENERGY) / REFERENCE_MIN_ENERGY;
    (range >= REFERENCE_MIN_RANGE).then_some(range)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_energy_produces_the_original_range() {
        let range = reference_range(1.0, REFERENCE_MIN_ENERGY);
        assert!(range.is_some_and(|range| (range - 254.0).abs() < 1.0e-4));
        let scaled = reference_range(0.02, REFERENCE_MIN_ENERGY);
        assert!(scaled.is_some_and(|range| (range - 4.1).abs() < 1.0e-4));
    }

    #[test]
    fn clipping_and_small_ranges_reject_virtual_lights() {
        assert_eq!(reference_range(0.01, 0.02), None);
        assert_eq!(reference_range(REFERENCE_MIN_ENERGY, 0.0), None);
        assert_eq!(reference_range(f32::NAN, 0.0), None);
        assert_eq!(reference_range(1.0, f32::NAN), None);
    }
}

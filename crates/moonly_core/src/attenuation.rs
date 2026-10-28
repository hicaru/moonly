//! Shared R4 local-light attenuation math.

/// Fraction of the full light range that carries attenuated energy.
pub const REFERENCE_ATTENUATION_RANGE_SCALE: f32 = 0.95;
const MIN_ATTENUATION_RANGE_METERS: f32 = 1.0e-4;

/// Return the inverse square of the padded attenuation range.
#[must_use]
pub fn reference_attenuation_factor(range: f32) -> f32 {
    let attenuation_range =
        (range.max(0.0) * REFERENCE_ATTENUATION_RANGE_SCALE).max(MIN_ATTENUATION_RANGE_METERS);
    1.0 / (attenuation_range * attenuation_range)
}

/// Return the R4 polynomial without a separate inverse-distance term.
#[must_use]
pub fn reference_attenuation(range: f32, distance: f32) -> f32 {
    let distance_squared = distance * distance;
    let linear = (1.0 - distance_squared * reference_attenuation_factor(range)).clamp(0.0, 1.0);
    linear * linear
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_curve_matches_center_midpoint_and_edge() {
        let cases = [(0.0, 1.0), (4.75, 0.5625), (9.5, 0.0), (10.0, 0.0)];
        for (distance, expected) in cases {
            assert!((reference_attenuation(10.0, distance) - expected).abs() < 1.0e-5);
        }
    }

    #[test]
    fn tiny_and_negative_ranges_stay_finite() {
        assert!(reference_attenuation_factor(0.0).is_finite());
        assert!(reference_attenuation_factor(-1.0).is_finite());
        assert!((reference_attenuation(0.0, 0.0) - 1.0).abs() < 1.0e-5);
        assert!(reference_attenuation(0.0, 1.0).abs() < 1.0e-5);
    }
}

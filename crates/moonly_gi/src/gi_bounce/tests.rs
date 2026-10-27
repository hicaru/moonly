use super::*;
use crate::gi_sampling::MONOLITH_GI_SEED;
use moonly_core::MoonlyLightType;

fn point_light() -> MoonlyLight {
    MoonlyLight {
        position: Vec3::ZERO,
        range: 10.0,
        flags: MoonlyLightFlags::GI_ENABLED,
        ..MoonlyLight::default()
    }
}

fn test_config() -> GiRadiosityConfig {
    GiRadiosityConfig {
        num_photons: 1,
        energy_clip: 0.0,
        ..GiRadiosityConfig::default()
    }
}

fn targeted_reflector(index: u32, distance: f32) -> (GiReflectorBox, Vec3) {
    let light = point_light();
    let mut random = MoonlyRandom::new();
    let mut target = Vec3::ZERO;
    for current in 0..=index {
        target = monolith_photon_dir(&light, &mut random).unwrap_or(Vec3::ZERO);
        assert!(current <= index);
    }
    (
        GiReflectorBox::from_center_extents(target * distance, Vec3::splat(0.2), Vec3::ONE),
        target,
    )
}

fn assert_vec_near(actual: Vec3, expected: Vec3) {
    assert!(
        (actual - expected).length() < 1.0e-4,
        "{actual} != {expected}"
    );
}

#[test]
fn monolith_sampling_uses_the_original_seed_and_sequence() {
    assert_eq!(MONOLITH_GI_SEED, 0x1207_1980);
    let light = point_light();
    let mut random = MoonlyRandom::new();
    let first = monolith_photon_dir(&light, &mut random).unwrap_or_default();
    assert_vec_near(first, Vec3::new(-0.160_599_9, 0.025_730_985, -0.986_684_14));
    let second = monolith_photon_dir(&light, &mut random).unwrap_or_default();
    assert_vec_near(second, Vec3::new(-0.072_750_12, -0.389_379_38, 0.918_199_9));
}

#[test]
fn monolith_traces_eight_candidates_per_retained_photon() {
    let (reflector, _) = targeted_reflector(7, 5.0);
    let mut out = Vec::new();
    gi_generate_monolith(&point_light(), &test_config(), &[reflector], &mut out);
    assert_eq!(out.len(), 1);

    let (late_reflector, _) = targeted_reflector(8, 5.0);
    out.clear();
    gi_generate_monolith(&point_light(), &test_config(), &[late_reflector], &mut out);
    assert!(out.is_empty());
}

#[test]
fn monolith_direction_is_reflected_and_position_is_not_offset() {
    let light = point_light();
    let (reflector, incident) = targeted_reflector(7, 5.0);
    let hit = raycast_all_reflectors(Vec3::ZERO, incident, light.range, 0.0, &[reflector]);
    let Some((distance, normal, _)) = hit else {
        return assert!(hit.is_some());
    };
    let cfg = GiRadiosityConfig {
        normal_standoff: 2.0,
        ..test_config()
    };
    let mut out = Vec::new();
    gi_generate_monolith(&light, &cfg, &[reflector], &mut out);
    let Some(virtual_light) = out.first() else {
        return assert!(!out.is_empty());
    };
    let reflected = incident - normal * (2.0 * incident.dot(normal));
    assert_vec_near(virtual_light.direction, reflected);
    assert_vec_near(virtual_light.position, incident * distance);
    assert!(virtual_light.direction.distance(normal) > 0.01);
}

#[test]
fn monolith_sorts_top_n_and_normalizes_total_linear_energy() {
    let light = point_light();
    let cfg = GiRadiosityConfig {
        num_photons: 2,
        reflectivity: 0.9,
        energy_clip: 0.0,
        ..test_config()
    };
    let candidate = |energy| VirtualGiLight {
        position: Vec3::ZERO,
        direction: Vec3::X,
        color: Vec3::splat(energy),
        range: 0.0,
        render_layers: light.render_layers,
    };
    let mut out = vec![candidate(0.2), candidate(0.6), candidate(0.4)];
    normalize_monolith_lights(&light, &cfg, 0, &mut out);
    assert_eq!(out.len(), 2);
    assert_vec_near(
        out.first().map(|v| v.color).unwrap_or_default(),
        Vec3::splat(0.54),
    );
    assert_vec_near(
        out.get(1).map(|v| v.color).unwrap_or_default(),
        Vec3::splat(0.36),
    );
    let total: f32 = out.iter().map(|v| v.color.x).sum();
    assert!((total - 0.9).abs() < 1.0e-5);
    let expected_range = 255.0 * 0.54 - 1.0;
    assert!((out.first().map(|v| v.range).unwrap_or_default() - expected_range).abs() < 1.0e-3);
}

#[test]
fn monolith_generation_reproduces_seeded_results_and_ignores_albedo() {
    let light = point_light();
    let (mut reflector, _) = targeted_reflector(7, 5.0);
    let cfg = test_config();
    let mut first = Vec::new();
    let mut second = Vec::new();
    gi_generate_monolith(&light, &cfg, &[reflector], &mut first);
    reflector.albedo = Vec3::ZERO;
    gi_generate_monolith(&light, &cfg, &[reflector], &mut second);
    assert_eq!(first, second);
    assert!(!first.is_empty());
}

#[test]
fn generators_reject_zero_photons_and_invalid_ranges() {
    let mut light = point_light();
    let (reflector, _) = targeted_reflector(7, 5.0);
    let mut cfg = test_config();
    for range in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        light.range = range;
        let mut monolith = Vec::new();
        let mut enhanced = Vec::new();
        let mut scratch = Vec::new();
        gi_generate_monolith(&light, &cfg, &[reflector], &mut monolith);
        gi_generate_enhanced(&light, &cfg, &[reflector], &mut enhanced, &mut scratch);
        assert!(monolith.is_empty());
        assert!(enhanced.is_empty());
    }
    light.range = 10.0;
    cfg.num_photons = 0;
    let mut out = Vec::new();
    gi_generate_monolith(&light, &cfg, &[reflector], &mut out);
    assert!(out.is_empty());
}

#[test]
fn direct_lights_do_not_emit_unsupported_monolith_photons() {
    let mut light = point_light();
    light.light_type = MoonlyLightType::Direct;
    let (reflector, _) = targeted_reflector(7, 5.0);
    let mut out = Vec::new();
    gi_generate_monolith(&light, &test_config(), &[reflector], &mut out);
    assert!(out.is_empty());
}

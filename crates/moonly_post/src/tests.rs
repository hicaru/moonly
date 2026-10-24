use moonly_core::{LightPackage, MoonlyConfig, ShadowFilterMode, WeatherEnvironment};
use moonly_gi::GiLightField;
use moonly_volumetrics::{SunShaftsConfig, SunShaftsMode};

use crate::{
    material::{LightingUniformParams, apply_lighting_uniform},
    *,
};

const EPSILON: f32 = 2.0e-5;

fn assert_vec3(actual: Vec3, expected: [f32; 3]) {
    let expected = Vec3::from(expected);
    assert!(
        actual.abs_diff_eq(expected, EPSILON),
        "actual {actual:?}, expected {expected:?}"
    );
}

#[test]
fn all_nine_tonemap_operators_match_golden_values() {
    let cases = [
        (
            TonemapOperator::ClassicXRay,
            [0.162_043_28, 0.771_940_9, 1.0],
        ),
        (
            TonemapOperator::ExtendedReinhardW2,
            [0.159_406_78, 0.709_090_9, 1.0],
        ),
        (
            TonemapOperator::ExtendedReinhardW3,
            [0.155_593_22, 0.618_181_8, 1.0],
        ),
        (
            TonemapOperator::AcesNarkowicz,
            [0.266_898_9, 0.839_712_9, 0.980_001],
        ),
        (
            TonemapOperator::AcesHill,
            [0.412_818_2, 0.683_118, 0.925_817_7],
        ),
        (
            TonemapOperator::Uchimura,
            [0.178_994_8, 0.887_706_3, 0.999_902_7],
        ),
        (
            TonemapOperator::SteveM,
            [0.197_492_17, 0.894_545_44, 0.988_363_6],
        ),
        (
            TonemapOperator::Uncharted2,
            [0.128_338_45, 0.549_548, 0.948_261_4],
        ),
        (
            TonemapOperator::AgX,
            [0.419_865_4, 0.664_579_6, 0.908_136_2],
        ),
    ];
    let hdr = Vec3::new(0.18, 1.2, 4.5);
    for (operator, expected) in cases {
        assert_vec3(evaluate_tonemap_cpu(hdr, operator), expected);
    }
}

#[test]
fn pq_encoding_matches_golden_values() {
    let encoded = encode_pq_st2084(Vec3::new(0.18, 1.0, 4.0), 1000.0);
    assert_vec3(encoded, [0.568_156_7, 0.751_827_1, 0.902_572_4]);
}

#[test]
fn karis_thirteen_sample_downsample_matches_normalized_golden() {
    let samples = std::array::from_fn(|idx| Vec3::splat((idx + 1) as f32 * 0.1));
    assert_vec3(karis_downsample_13(samples), [0.581_343_65; 3]);
    let groups = [0.3, 0.4, 0.6, 0.7, 1.15].map(Vec3::splat);
    assert_vec3(karis_group5_average(groups), [0.581_343_65; 3]);
}

#[test]
fn karis_single_group_still_suppresses_extreme_firefly() {
    let avg = karis_box4_average(
        Vec3::splat(1000.0),
        Vec3::splat(0.1),
        Vec3::splat(0.1),
        Vec3::splat(0.1),
    );
    assert!(avg.max_element() < 2.0);
}

#[test]
fn optics_match_golden_values() {
    let rgb = Vec3::new(0.18, 0.6, 1.4);
    let uv = Vec2::new(0.37, 0.61);
    assert_vec3(
        evaluate_night_vision_cpu(rgb, uv, false),
        [0.261_096_7, 1.392_515_7, 0.406_150_4],
    );
    assert_vec3(
        evaluate_night_vision_cpu(rgb, uv, true),
        [0.986_365_3, 1.334_494_2, 1.421_526_5],
    );
    assert_vec3(
        evaluate_heat_vision_cpu(rgb),
        [0.808_872_8, 0.242_367_2, 0.484_565_2],
    );
    assert_vec3(
        evaluate_lens_flare_ghost(uv, Vec2::new(0.62, 0.44), 0.31, 0.42),
        [0.277_492_94, 0.252_266_3, 0.227_039_68],
    );
}

#[test]
fn absent_lights_reset_all_light_uniforms_to_neutral() {
    let mut uniform = MoonlyLitUniform {
        spot_pos_range: Vec4::ONE,
        spot_dir_cos: Vec4::ONE,
        spot_color_vol: Vec4::ONE,
        gi_pos_range_0: Vec4::ONE,
        gi_color_0: Vec4::ONE,
        gi_pos_range_1: Vec4::ONE,
        gi_color_1: Vec4::ONE,
        gi_pos_range_2: Vec4::ONE,
        gi_color_2: Vec4::ONE,
        ..MoonlyLitUniform::default()
    };
    apply_test_uniform(
        &mut uniform,
        ShadowFilterMode::MonolithJitter64,
        SunShaftsMode::Combined,
    );
    let light_values = [
        uniform.spot_pos_range,
        uniform.spot_dir_cos,
        uniform.spot_color_vol,
        uniform.gi_pos_range_0,
        uniform.gi_color_0,
        uniform.gi_pos_range_1,
        uniform.gi_color_1,
        uniform.gi_pos_range_2,
        uniform.gi_color_2,
    ];
    assert!(light_values.into_iter().all(|value| value == Vec4::ZERO));
    assert_eq!(MoonlyLitUniform::default().spot_color_vol, Vec4::ZERO);
}

#[test]
fn shadow_filter_and_shaft_parameters_sync_without_constant_substitution() {
    for (filter, expected) in [
        (ShadowFilterMode::HardwarePcf, 0),
        (ShadowFilterMode::MonolithJitter64, 1),
        (ShadowFilterMode::MonolithMinMax, 2),
        (ShadowFilterMode::EnhancedPcss, 3),
    ] {
        let mut uniform = MoonlyLitUniform::default();
        apply_test_uniform(&mut uniform, filter, SunShaftsMode::Off);
        assert_eq!(uniform.mode_flags.z, expected);
        assert!(uniform.shaft_params.x.abs() < EPSILON);
        assert!(uniform.post_params.w.abs() < EPSILON);
        assert!((uniform.shaft_params.y - 0.68).abs() < EPSILON);
        assert!((uniform.shaft_params.z - 0.96).abs() < EPSILON);
        assert!((uniform.shaft_params.w - 24.0).abs() < EPSILON);
    }
}

fn apply_test_uniform(
    uniform: &mut MoonlyLitUniform,
    filter: ShadowFilterMode,
    shaft_mode: SunShaftsMode,
) {
    let config = MoonlyConfig {
        shadow_filter: filter,
        ..MoonlyConfig::default()
    };
    let env = WeatherEnvironment::default();
    let package = LightPackage::default();
    let gi_field = GiLightField::default();
    let shafts = SunShaftsConfig {
        mode: shaft_mode,
        ..SunShaftsConfig::default()
    };
    let hdr10 = Hdr10PostConfig::default();
    let optics = OpticsMode::Standard;
    apply_lighting_uniform(
        uniform,
        &LightingUniformParams {
            config: &config,
            env: &env,
            package: &package,
            gi_field: &gi_field,
            shafts: &shafts,
            hdr10: &hdr10,
            optics: &optics,
        },
    );
}

#[test]
fn dual_wave_gaussian_weights_sum_to_unity() {
    let weights = calc_gauss_dual_wave(0.35);
    let mut total = weights.first().copied().unwrap_or(0.0);
    for &tap in weights.get(1..).unwrap_or(&[]) {
        total += 2.0 * tap;
    }
    assert!((total - 1.0).abs() < 1.0e-5);
}

#[test]
fn lit_shader_uses_shared_dispatch_and_required_gates() {
    for index in 0..=8 {
        assert!(TONEMAP_WESL_SOURCE.contains(&format!("if op_idx == {index}u")));
    }
    assert!(MOONLY_LIT_WESL_SOURCE.contains("evaluate_hdr10_tonemap(hdr, mat.mode_flags.x)"));
    assert!(MOONLY_LIT_WESL_SOURCE.contains("mat.mode_flags.w != 0u"));
    assert!(MOONLY_LIT_WESL_SOURCE.contains("mat.shaft_params.x"));
    assert!(MOONLY_LIT_WESL_SOURCE.contains("mat.shaft_params.y"));
    assert!(MOONLY_LIT_WESL_SOURCE.contains("apply_contrast(mapped, mat.post_params.y)"));
    assert!(MOONLY_LIT_WESL_SOURCE.contains("normalize_safe(mat.spot_dir_cos.xyz)"));
    assert_eq!(MOONLY_LIT_WESL_SOURCE.matches("sun_ndotl > 0.0").count(), 1);
    assert_eq!(
        MOONLY_LIT_WESL_SOURCE.matches("spot_ndotl > 0.0").count(),
        1
    );
}

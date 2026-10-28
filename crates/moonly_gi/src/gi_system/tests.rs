use super::*;
use moonly_core::{MoonlyLightFlags, MoonlyPipelineTier};

fn floor() -> GiReflectorBox {
    GiReflectorBox::from_center_extents(
        Vec3::new(0.0, -0.25, 0.0),
        Vec3::new(20.0, 0.25, 20.0),
        Vec3::splat(0.8),
    )
}

fn config(tier: MoonlyPipelineTier) -> MoonlyConfig {
    let mut config = MoonlyConfig {
        tier,
        ..MoonlyConfig::default()
    };
    config.gi.energy_clip = 0.0;
    config
}

fn generate(config: &MoonlyConfig, package: &LightPackage) -> GiLightField {
    let mut field = GiLightField::default();
    let mut scratch = Vec::new();
    generate_package_gi(config, package, &[floor()], &mut field, &mut scratch);
    field
}

#[test]
fn unshadowed_point_and_spot_lights_generate_gi() {
    let mut package = LightPackage::default();
    let mut point = MoonlyLight {
        position: Vec3::new(0.0, 2.0, 0.0),
        flags: MoonlyLightFlags::GI_ENABLED,
        ..MoonlyLight::default()
    };
    package.v_point.push(point.clone());
    let field = generate(&config(MoonlyPipelineTier::MonolithR4), &package);
    assert_ne!(field.virtual_lights.len(), 0);

    package.v_point.clear();
    point.light_type = MoonlyLightType::Spot;
    point.direction = Vec3::NEG_Y;
    point.right = Vec3::X;
    point.cone_angle = 0.8;
    package.v_spot.push(point);
    let field = generate(&config(MoonlyPipelineTier::MonolithR4), &package);
    assert_ne!(field.virtual_lights.len(), 0);
}

#[test]
fn split_shadowed_point_matches_one_unshadowed_point() {
    let point = MoonlyLight {
        position: Vec3::new(0.0, 2.0, 0.0),
        flags: MoonlyLightFlags::SHADOW.union(MoonlyLightFlags::GI_ENABLED),
        ..MoonlyLight::default()
    };
    let mut shadowed = LightPackage::default();
    shadowed.push(point.clone());
    assert_eq!(shadowed.v_shadowed.len(), 6);

    let mut unshadowed = LightPackage::default();
    unshadowed.v_point.push(point);
    for tier in [
        MoonlyPipelineTier::MonolithR4,
        MoonlyPipelineTier::MoonlyEnhanced,
    ] {
        let config = config(tier);
        let split_field = generate(&config, &shadowed);
        let point_field = generate(&config, &unshadowed);
        assert_ne!(split_field.virtual_lights.len(), 0);
        assert_eq!(split_field.virtual_lights, point_field.virtual_lights);
    }
}

#[test]
fn disabled_and_non_gi_package_lights_do_not_generate() {
    let mut package = LightPackage::default();
    package.v_point.push(MoonlyLight {
        flags: MoonlyLightFlags::default(),
        ..MoonlyLight::default()
    });
    let field = generate(&config(MoonlyPipelineTier::MonolithR4), &package);
    assert_eq!(field.virtual_lights.len(), 0);

    package.v_point.clear();
    package.v_point.push(MoonlyLight {
        flags: MoonlyLightFlags::GI_ENABLED,
        ..MoonlyLight::default()
    });
    let mut config = config(MoonlyPipelineTier::MonolithR4);
    config.gi.enabled = false;
    let field = generate(&config, &package);
    assert_eq!(field.virtual_lights.len(), 0);
}

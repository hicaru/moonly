use super::*;
use moonly_core::{MoonlyPipelineTier, XRayLightFlags};

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
    let mut point = XRayLight {
        position: Vec3::new(0.0, 2.0, 0.0),
        flags: XRayLightFlags::GI_ENABLED,
        ..XRayLight::default()
    };
    package.v_point.push(point.clone());
    let field = generate(&config(MoonlyPipelineTier::MonolithR4), &package);
    assert!(!field.virtual_lights.is_empty());

    package.v_point.clear();
    point.light_type = XRayLightType::Spot;
    point.direction = Vec3::NEG_Y;
    point.right = Vec3::X;
    point.cone_angle = 0.8;
    package.v_spot.push(point);
    let field = generate(&config(MoonlyPipelineTier::MonolithR4), &package);
    assert!(!field.virtual_lights.is_empty());
}

#[test]
fn split_shadowed_point_matches_one_unshadowed_point() {
    let point = XRayLight {
        position: Vec3::new(0.0, 2.0, 0.0),
        flags: XRayLightFlags::SHADOW.union(XRayLightFlags::GI_ENABLED),
        ..XRayLight::default()
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
        assert!(!split_field.virtual_lights.is_empty());
        assert_eq!(split_field.virtual_lights, point_field.virtual_lights);
    }
}

#[test]
fn disabled_and_non_gi_package_lights_do_not_generate() {
    let mut package = LightPackage::default();
    package.v_point.push(XRayLight {
        flags: XRayLightFlags::default(),
        ..XRayLight::default()
    });
    let field = generate(&config(MoonlyPipelineTier::MonolithR4), &package);
    assert!(field.virtual_lights.is_empty());

    package.v_point.clear();
    package.v_point.push(XRayLight {
        flags: XRayLightFlags::GI_ENABLED,
        ..XRayLight::default()
    });
    let mut config = config(MoonlyPipelineTier::MonolithR4);
    config.gi.enabled = false;
    let field = generate(&config, &package);
    assert!(field.virtual_lights.is_empty());
}

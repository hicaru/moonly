use super::*;
use crate::{LightPackage, collect_light_packages};

fn native_test_app() -> App {
    let mut app = App::new();
    app.init_resource::<LightPackage>().add_systems(
        Update,
        (
            remove_native_light_adapters,
            sync_native_point_lights,
            sync_native_spot_lights,
            sync_native_directional_lights,
            collect_light_packages,
        )
            .chain(),
    );
    app
}

#[test]
fn point_light_preserves_lumens_updates_and_removal() {
    let mut app = native_test_app();
    let entity = app
        .world_mut()
        .spawn((
            PointLight {
                intensity: FULL_SPHERE_STERADIANS,
                range: 23.0,
                shadow_maps_enabled: false,
                ..PointLight::default()
            },
            InheritedVisibility::VISIBLE,
        ))
        .id();
    app.update();
    let adapted = app.world().get::<MoonlyLight>(entity);
    assert!(adapted.is_some_and(|light| {
        light.color.abs_diff_eq(Vec3::ONE, 1.0e-5)
            && (light.range - 23.0).abs() < 1.0e-5
            && !light.flags.contains(MoonlyLightFlags::SHADOW)
    }));
    assert_eq!(app.world().resource::<LightPackage>().v_point.len(), 1);

    if let Some(mut light) = app.world_mut().get_mut::<PointLight>(entity) {
        light.intensity *= 2.0;
    }
    app.update();
    assert!(
        app.world()
            .get::<MoonlyLight>(entity)
            .is_some_and(|light| { light.color.abs_diff_eq(Vec3::splat(2.0), 1.0e-5) })
    );

    app.world_mut().entity_mut(entity).remove::<PointLight>();
    app.update();
    assert!(app.world().get::<MoonlyLight>(entity).is_none());
    assert_eq!(app.world().resource::<LightPackage>().total_count(), 0);
}

#[test]
fn explicit_moonly_light_has_priority_over_native_adapter() {
    let mut app = native_test_app();
    let explicit = MoonlyLight {
        color: Vec3::splat(7.0),
        ..MoonlyLight::default()
    };
    let entity = app
        .world_mut()
        .spawn((PointLight::default(), explicit))
        .id();
    app.update();
    assert!(
        app.world()
            .get::<MoonlyLight>(entity)
            .is_some_and(|light| light.color == Vec3::splat(7.0))
    );

    app.world_mut().entity_mut(entity).remove::<PointLight>();
    app.update();
    assert!(app.world().get::<MoonlyLight>(entity).is_some());
}

#[test]
fn spot_light_preserves_cone_angles_and_shadow_flags() {
    let mut app = native_test_app();
    let entity = app
        .world_mut()
        .spawn(SpotLight {
            intensity: FULL_SPHERE_STERADIANS * 2.0,
            range: 31.0,
            inner_angle: 0.2,
            outer_angle: 0.6,
            shadow_maps_enabled: true,
            ..SpotLight::default()
        })
        .id();
    app.update();
    assert!(app.world().get::<MoonlyLight>(entity).is_some_and(|light| {
        light.color.abs_diff_eq(Vec3::splat(2.0), 1.0e-5)
            && (light.range - 31.0).abs() < 1.0e-5
            && (light.inner_cone_angle - 0.2).abs() < 1.0e-5
            && (light.cone_angle - 0.6).abs() < 1.0e-5
            && light.flags.contains(MoonlyLightFlags::SHADOW)
    }));
}

#[test]
fn directional_light_preserves_illuminance() {
    let mut app = native_test_app();
    let entity = app
        .world_mut()
        .spawn(DirectionalLight {
            illuminance: 2500.0,
            ..DirectionalLight::default()
        })
        .id();
    app.update();
    assert!(app.world().get::<MoonlyLight>(entity).is_some_and(|light| {
        light.light_type == MoonlyLightType::Direct && light.color == Vec3::splat(2500.0)
    }));
}

#[test]
fn hidden_light_is_excluded_from_the_package() {
    let mut app = native_test_app();
    app.world_mut()
        .spawn((PointLight::default(), InheritedVisibility::HIDDEN));
    app.update();
    assert_eq!(app.world().resource::<LightPackage>().total_count(), 0);
}

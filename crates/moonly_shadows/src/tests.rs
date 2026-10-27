use moonly_core::{
    LightPackage, MoonlyConfig, MoonlyLight, MoonlyLightFlags, MoonlyLightType, SunCascadeConfig,
    WeatherEnvironment,
};

use super::*;

fn shadow_app(config: MoonlyConfig) -> App {
    let mut app = App::new();
    app.insert_resource(config)
        .init_resource::<WeatherEnvironment>()
        .init_resource::<LightPackage>()
        .init_resource::<SmapAllocator>()
        .init_resource::<LocalShadowBatches>()
        .init_resource::<SunCascades>()
        .init_resource::<RainShadowState>()
        .init_resource::<moonly_core::MoonlyNativeForward>()
        .init_resource::<bevy::light::DirectionalLightShadowMap>()
        .add_systems(Update, update_shadow_cascades_and_atlas);
    app
}

fn spawn_camera(app: &mut App, cam: CameraFrustumParams, is_active: bool, order: isize) -> Entity {
    app.world_mut()
        .spawn((
            Camera {
                is_active,
                order,
                ..Camera::default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: cam.fov_y_radians,
                aspect_ratio: cam.aspect_ratio,
                near: cam.z_near,
                ..PerspectiveProjection::default()
            }),
            GlobalTransform::from(cam.camera_transform),
        ))
        .id()
}

fn spot_with_size(size: u16) -> MoonlyLight {
    MoonlyLight {
        light_type: MoonlyLightType::Spot,
        flags: MoonlyLightFlags::SHADOW,
        smap_size_override: size,
        ..MoonlyLight::default()
    }
}

#[test]
fn monolith_allocator_matches_golden_full_atlas_coordinates() {
    let mut alloc = SmapAllocator::new(64);
    for _ in 0..4 {
        assert!(alloc.push_monolith(32).is_some());
    }
    assert_eq!(
        alloc.allocated,
        [
            SmapRect::new(0, 0, 31, 31),
            SmapRect::new(32, 0, 63, 31),
            SmapRect::new(0, 32, 31, 63),
            SmapRect::new(32, 32, 63, 63),
        ]
    );
}

#[test]
fn monolith_allocator_uses_critical_point_insertion_order() {
    let mut alloc = SmapAllocator::new(64);
    for _ in 0..5 {
        assert!(alloc.push_monolith(16).is_some());
    }
    assert_eq!(
        alloc.allocated,
        [
            SmapRect::new(0, 0, 15, 15),
            SmapRect::new(16, 0, 31, 15),
            SmapRect::new(0, 16, 15, 31),
            SmapRect::new(32, 0, 47, 15),
            SmapRect::new(16, 16, 31, 31),
        ]
    );
}

#[test]
fn monolith_allocator_rejects_invalid_sizes_and_reports_exhaustion() {
    let mut alloc = SmapAllocator::new(64);
    assert_eq!(alloc.push_monolith(0), None);
    assert_eq!(alloc.push_monolith(4), None);
    assert_eq!(alloc.push_monolith(65), None);
    assert_eq!(alloc.failed_allocations, 3);
    for _ in 0..4 {
        assert!(alloc.push_monolith(32).is_some());
    }
    let allocated = alloc.allocated.clone();
    assert_eq!(alloc.push_monolith(5), None);
    assert_eq!(alloc.allocated, allocated);
    assert_eq!(alloc.failed_allocations, 4);
}

#[test]
fn texel_adjust_matrix_matches_r4_half_texel_offset() {
    let rect = SmapRect::new(64, 128, 321, 385);
    let matrix = rect.texel_adjust_matrix(258, 1024, -0.02);
    let expected = Mat4::from_cols(
        Vec4::new(0.125, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -0.125, 0.0, 0.0),
        Vec4::new(0.0, 0.0, 1.0, 0.0),
        Vec4::new(0.188_964_84, 0.251_464_84, -0.02, 1.0),
    );
    assert!(matrix.abs_diff_eq(expected, 1.0e-7));
}

#[test]
fn bssf_allocator_packs_mixed_sizes_efficiently() {
    let mut alloc = SmapAllocator::new(1024);
    for size in [512, 256, 256, 512, 256, 128, 128] {
        assert!(alloc.push_enhanced_bssf(size).is_some());
    }
    assert_eq!(alloc.failed_allocations, 0);
}

#[test]
fn spot_shadow_xform_applies_one_percent_hysteresis() {
    let light = MoonlyLight::spot(Vec3::new(0.0, 4.0, 0.0), Vec3::NEG_Y, Vec3::ONE, 15.0, 1.0);
    let first = SpotShadowXform::compute_xf_spot(&light, Vec3::ZERO, 2048, 0);
    let second =
        SpotShadowXform::compute_xf_spot(&light, Vec3::new(0.01, 0.0, 0.0), 2048, first.smap_size);
    assert_eq!(first.smap_size, second.smap_size);
}

#[test]
fn sun_cascades_produce_valid_orthographic_slices() {
    let mut cascades = SunCascades::default();
    let cam = CameraFrustumParams::default();
    let cfg = SunCascadeConfig::default();
    cascades.compute(&cam, Vec3::new(-0.4, -0.8, -0.4).normalize(), &cfg, 2048);
    for slice in &cascades.cascades {
        let midpoint = cam.camera_transform.translation
            + cam.camera_transform.forward() * f32::midpoint(slice.split_near, slice.split_far);
        let ndc = slice.combine.project_point3(midpoint);
        assert!(slice.split_far > slice.split_near);
        assert!(slice.combine.determinant().abs() > 0.0);
        assert!(ndc.abs().cmple(Vec3::ONE).all());
    }
}

fn assert_cascades_approx(actual: &SunCascades, expected: &SunCascades) {
    for (actual, expected) in actual.cascades.iter().zip(expected.cascades.iter()) {
        assert!(actual.view.abs_diff_eq(expected.view, 1.0e-4));
        assert!(actual.proj.abs_diff_eq(expected.proj, 1.0e-4));
        assert!(actual.combine.abs_diff_eq(expected.combine, 1.0e-4));
        assert!((actual.split_near - expected.split_near).abs() < f32::EPSILON);
        assert!((actual.split_far - expected.split_far).abs() < f32::EPSILON);
    }
}

#[test]
fn shadow_update_uses_active_camera_and_computes_rain() {
    let config = MoonlyConfig {
        rain_wetness: 0.5,
        ..MoonlyConfig::default()
    };
    let mut app = shadow_app(config.clone());
    let selected = CameraFrustumParams {
        camera_transform: Transform::from_xyz(10.0, 3.0, -4.0)
            .with_rotation(Quat::from_rotation_y(0.4)),
        fov_y_radians: 1.1,
        aspect_ratio: 1.5,
        z_near: 0.25,
    };
    spawn_camera(&mut app, CameraFrustumParams::default(), false, 10);
    spawn_camera(&mut app, selected, true, 2);
    app.world_mut()
        .resource_mut::<LightPackage>()
        .push(MoonlyLight {
            light_type: MoonlyLightType::Direct,
            direction: Vec3::NEG_Y,
            ..MoonlyLight::default()
        });

    app.update();

    let mut expected = SunCascades::default();
    expected.compute(
        &selected,
        Vec3::NEG_Y,
        &config.sun_cascades,
        config.smap_pool_size,
    );
    assert_cascades_approx(app.world().resource::<SunCascades>(), &expected);
    let rain = RainShadowState::compute(
        selected.camera_transform.translation,
        selected.camera_transform.forward().as_vec3(),
        RainShadowState::default().rain_direction,
        selected.fov_y_radians,
        selected.aspect_ratio,
    );
    let actual_rain = app.world().resource::<RainShadowState>();
    assert!(actual_rain.combine.abs_diff_eq(rain.combine, 1.0e-5));
    assert!(actual_rain.sphere_center.distance(rain.sphere_center) < 1.0e-5);
}

#[test]
fn shadow_update_preserves_previous_smap_size_for_hysteresis() {
    let mut app = shadow_app(MoonlyConfig::default());
    let light = MoonlyLight::spot(Vec3::ZERO, Vec3::NEG_Y, Vec3::ONE, 15.0, 1.0);
    app.world_mut()
        .resource_mut::<LightPackage>()
        .push(light.clone());
    let camera = spawn_camera(
        &mut app,
        CameraFrustumParams {
            camera_transform: Transform::IDENTITY,
            ..CameraFrustumParams::default()
        },
        true,
        0,
    );
    app.update();
    let first = app
        .world()
        .resource::<LocalShadowBatches>()
        .active()
        .first()
        .and_then(|batch| batch.views.first())
        .map(|view| view.rect.width());
    assert_eq!(first, Some(1024));

    app.world_mut()
        .entity_mut(camera)
        .insert(GlobalTransform::from(Transform::from_xyz(15.02, 0.0, 0.0)));
    app.update();

    let second = app
        .world()
        .resource::<LocalShadowBatches>()
        .active()
        .first()
        .and_then(|batch| batch.views.first())
        .map(|view| view.rect.width());
    assert_eq!(second, first);
    let without_history =
        SpotShadowXform::compute_xf_spot(&light, Vec3::new(15.02, 0.0, 0.0), 2048, 0);
    assert_ne!(Some(without_history.smap_size), first);
}

#[test]
fn shadow_update_overflow_creates_additional_batches() {
    let config = MoonlyConfig {
        smap_pool_size: 128,
        ..MoonlyConfig::default()
    };
    let mut app = shadow_app(config);
    for _ in 0..5 {
        app.world_mut()
            .resource_mut::<LightPackage>()
            .push(spot_with_size(64));
    }
    spawn_camera(&mut app, CameraFrustumParams::default(), true, 0);
    app.update();

    let batches = app.world().resource::<LocalShadowBatches>();
    assert_eq!(batches.view_count(), 5);
    assert!(batches.batch_count() > 1);
}

#[test]
fn shadow_update_does_not_substitute_a_default_camera() {
    let mut app = shadow_app(MoonlyConfig::default());
    app.world_mut()
        .resource_mut::<LightPackage>()
        .push(spot_with_size(64));
    app.world_mut().insert_resource(SmapAllocator::new(128));
    let before = app.world().resource::<SunCascades>().clone();
    app.update();

    assert_eq!(app.world().resource::<SunCascades>(), &before);
    assert!(app.world().resource::<SmapAllocator>().allocated.is_empty());
}

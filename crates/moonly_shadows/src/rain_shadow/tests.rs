use super::*;

fn params(camera: CameraFrustumParams) -> RainComputeParams {
    RainComputeParams {
        camera,
        rain_direction: Vec3::NEG_Y,
        config: RainWetSurfaceConfig::default(),
        pool_size: 2048,
    }
}

#[test]
fn bounding_radius_has_reference_margin_and_minimum() {
    let fov = 1.1_f32;
    let aspect = 2.0_f32;
    let a = (0.5 * fov).tan();
    let c = (0.5 * fov * aspect).tan();
    let expected = 30.0 * (1.0 + a * a + c * c) * 0.5 * 1.05;
    assert!((bounding_radius(fov, aspect, 30.0) - expected).abs() < 1.0e-4);
    assert_eq!(bounding_radius(fov, aspect, 0.0), 0.1);
}

#[test]
fn camera_height_does_not_change_horizontal_footprint_offset() {
    let low = footprint_center(Vec3::ZERO, Vec3::new(0.5, 0.8, -0.2), Vec3::NEG_Y, 20.0);
    let high = footprint_center(
        Vec3::new(0.0, 100.0, 0.0),
        Vec3::new(0.5, 0.8, -0.2),
        Vec3::NEG_Y,
        20.0,
    );
    assert!((low.x - high.x).abs() < f32::EPSILON);
    assert!((low.z - high.z).abs() < f32::EPSILON);
}

#[test]
fn light_depth_preserves_reference_asymmetry() {
    let (near, far) = light_space_depth_range(-30.0, -10.0);
    assert_eq!(near, -990.0);
    assert_eq!(far, 2030.0);
}

#[test]
fn state_uses_the_effective_resolution() {
    let mut compute = params(CameraFrustumParams::default());
    compute.config.smap_resolution = 512;
    compute.pool_size = 128;
    let state = RainShadowState::compute(compute);
    assert_eq!(state.smap_size, 128);
    assert_eq!(state.generation, 1);
}

#[test]
fn sub_texel_motion_inside_a_grid_cell_preserves_the_matrix() {
    let camera = CameraFrustumParams {
        camera_transform: Transform::from_xyz(0.003, 2.0, 8.0),
        ..CameraFrustumParams::default()
    };
    let moved = CameraFrustumParams {
        camera_transform: Transform::from_xyz(0.0031, 2.0, 8.0),
        ..camera
    };
    let first = RainShadowState::compute(params(camera));
    let second = RainShadowState::compute(params(moved));
    assert!(
        first.combine.abs_diff_eq(second.combine, 1.0e-4),
        "{:?} != {:?}",
        first.combine,
        second.combine
    );
}

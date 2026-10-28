use super::*;
use crate::{cascade_planes::extract_frustum_planes, spot_xform::perspective_rh_mat4};

#[test]
fn cull_near_plane_uses_zero_to_one_clip_depth() {
    let projection = perspective_rh_mat4(1.0, 1.5, 0.1, 100.0);
    let planes = extract_frustum_planes(projection);
    let Some(near) = planes.get(4) else {
        return;
    };
    let before_near = Vec3::new(0.0, 0.0, -0.05).extend(1.0);
    let at_near = Vec3::new(0.0, 0.0, -0.1).extend(1.0);
    let inside = Vec3::new(0.0, 0.0, -1.0).extend(1.0);
    assert!(near.dot(before_near) < 0.0);
    assert!(near.dot(at_near).abs() < 1.0e-5);
    assert!(near.dot(inside) > 0.0);
}

#[test]
fn blend_weight_covers_the_complete_transition() {
    assert!(cascade_blend_weight(19.0, 20.0, 4.0).abs() < f32::EPSILON);
    assert!((cascade_blend_weight(22.0, 20.0, 4.0) - 0.5).abs() < 1.0e-6);
    assert!((cascade_blend_weight(24.0, 20.0, 4.0) - 1.0).abs() < f32::EPSILON);
    assert!((cascade_blend_weight(25.0, 20.0, 4.0) - 1.0).abs() < f32::EPSILON);
}

#[test]
fn previous_spatial_cascade_starts_with_the_near_matrix() {
    let cascades = SunCascadeSlice {
        combine: Mat4::from_scale(Vec3::splat(2.0)),
        ..SunCascadeSlice::default()
    };
    let all = [cascades; SUN_CASCADE_COUNT];
    assert_eq!(previous_cascade_combine(&all, 0), cascades.combine);
    assert_eq!(previous_cascade_combine(&all, 1), cascades.combine);
}

#[test]
fn sub_texel_translation_inside_a_grid_cell_preserves_snapped_matrices() {
    for footprint in [
        SunCascadeFootprint::FrustumFit,
        SunCascadeFootprint::ReferenceFixed,
    ] {
        let config = SunCascadeConfig {
            footprint,
            ..SunCascadeConfig::reference()
        };
        let mut first = SunCascades::default();
        let mut second = SunCascades::default();
        let camera = CameraFrustumParams {
            camera_transform: Transform::from_xyz(0.003, 2.0, 8.0),
            ..CameraFrustumParams::default()
        };
        let moved = CameraFrustumParams {
            camera_transform: Transform::from_translation(
                camera.camera_transform.translation + Vec3::new(0.0001, 0.0, 0.0),
            ),
            ..camera
        };
        first.compute(&camera, Vec3::NEG_Y, &config, 2048);
        second.compute(&moved, Vec3::NEG_Y, &config, 2048);
        for (index, (first, second)) in first
            .cascades
            .iter()
            .zip(second.cascades.iter())
            .enumerate()
        {
            assert!(
                first.combine.abs_diff_eq(second.combine, 1.0e-6),
                "{footprint:?} cascade {index}: {:?} != {:?}",
                first.combine,
                second.combine
            );
        }
    }
}

#[test]
fn fixed_policy_expands_for_wide_frustum_coverage() {
    let config = SunCascadeConfig {
        ortho_sizes: [1.0, 1.0, 1.0],
        ..SunCascadeConfig::reference()
    };
    let camera = CameraFrustumParams {
        fov_y_radians: 2.4,
        aspect_ratio: 2.0,
        ..CameraFrustumParams::default()
    };
    let mut cascades = SunCascades::default();
    cascades.compute(&camera, Vec3::NEG_Y, &config, 2048);
    for slice in &cascades.cascades {
        let midpoint = camera.camera_transform.translation
            + camera.camera_transform.forward() * f32::midpoint(slice.split_near, slice.split_far);
        let ndc = slice.combine.project_point3(midpoint);
        assert!(slice.footprint_size > 1.0);
        assert!(ndc.abs().cmple(Vec3::ONE).all());
    }
}

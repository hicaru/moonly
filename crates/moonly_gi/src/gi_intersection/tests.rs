use super::*;

fn unit_box() -> (Vec3, Vec3) {
    (Vec3::splat(-1.0), Vec3::splat(1.0))
}

fn assert_hit(hit: Option<(f32, Vec3)>, distance: f32, normal: Vec3) {
    assert!(hit.is_some());
    let (actual_distance, actual_normal) = hit.unwrap_or_default();
    assert!((actual_distance - distance).abs() < 1.0e-5);
    assert_eq!(actual_normal, normal);
}

#[test]
fn outside_rays_report_each_entry_face() {
    let (min, max) = unit_box();
    let cases = [
        (Vec3::new(-2.0, 0.0, 0.0), Vec3::X, Vec3::NEG_X),
        (Vec3::new(2.0, 0.0, 0.0), Vec3::NEG_X, Vec3::X),
        (Vec3::new(0.0, -2.0, 0.0), Vec3::Y, Vec3::NEG_Y),
        (Vec3::new(0.0, 2.0, 0.0), Vec3::NEG_Y, Vec3::Y),
        (Vec3::new(0.0, 0.0, -2.0), Vec3::Z, Vec3::NEG_Z),
        (Vec3::new(0.0, 0.0, 2.0), Vec3::NEG_Z, Vec3::Z),
    ];
    for (origin, dir, normal) in cases {
        assert_hit(intersect_ray_aabb(origin, dir, min, max), 1.0, normal);
    }
}

#[test]
fn inside_rays_report_the_exit_face_normal() {
    let (min, max) = unit_box();
    let cases = [
        (Vec3::X, 1.0, Vec3::X),
        (Vec3::NEG_X, 1.0, Vec3::NEG_X),
        (Vec3::Y, 1.0, Vec3::Y),
        (Vec3::NEG_Y, 1.0, Vec3::NEG_Y),
        (Vec3::Z, 1.0, Vec3::Z),
        (Vec3::NEG_Z, 1.0, Vec3::NEG_Z),
    ];
    for (dir, distance, normal) in cases {
        assert_hit(
            intersect_ray_aabb(Vec3::ZERO, dir, min, max),
            distance,
            normal,
        );
    }
    let min = Vec3::splat(-1.0);
    let max = Vec3::new(3.0, 1.0, 1.0);
    assert_hit(
        intersect_ray_aabb(Vec3::ZERO, Vec3::X, min, max),
        3.0,
        Vec3::X,
    );
}

#[test]
fn parallel_boundary_and_tangent_rays_follow_slab_rules() {
    let (min, max) = unit_box();
    assert_eq!(
        intersect_ray_aabb(Vec3::new(-2.0, 2.0, 0.0), Vec3::X, min, max),
        None
    );
    assert_eq!(
        intersect_ray_aabb(Vec3::new(-2.0, 0.0, 0.0), Vec3::ZERO, min, max),
        None
    );
    assert_eq!(intersect_ray_aabb(Vec3::ZERO, Vec3::ZERO, min, max), None);
    assert_hit(
        intersect_ray_aabb(Vec3::new(-2.0, -1.0, 0.0), Vec3::X, min, max),
        1.0,
        Vec3::NEG_X,
    );
    let corner = intersect_ray_aabb(
        Vec3::new(-2.0, -2.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        min,
        max,
    );
    assert!((corner.map(|hit| hit.0).unwrap_or_default() - 1.0).abs() < 1.0e-5);
}

#[test]
fn boundary_origins_and_boxes_behind_the_ray_are_handled() {
    let (min, max) = unit_box();
    assert_hit(
        intersect_ray_aabb(Vec3::new(-1.0, 0.0, 0.0), Vec3::X, min, max),
        0.0,
        Vec3::NEG_X,
    );
    assert_hit(
        intersect_ray_aabb(Vec3::new(-1.0, 0.0, 0.0), Vec3::NEG_X, min, max),
        0.0,
        Vec3::NEG_X,
    );
    assert_eq!(
        intersect_ray_aabb(Vec3::new(2.0, 0.0, 0.0), Vec3::X, min, max),
        None
    );
    assert_hit(
        intersect_ray_aabb(Vec3::NEG_X, Vec3::X, Vec3::ZERO, Vec3::ZERO),
        1.0,
        Vec3::NEG_X,
    );
}

#[test]
fn invalid_aabbs_and_directions_do_not_hit() {
    let (min, max) = unit_box();
    assert_eq!(intersect_ray_aabb(Vec3::ZERO, Vec3::X, max, min), None);
    assert_eq!(intersect_ray_aabb(Vec3::NAN, Vec3::X, min, max), None);
    assert_eq!(intersect_ray_aabb(Vec3::ZERO, Vec3::NAN, min, max), None);
    assert_eq!(
        intersect_ray_aabb(Vec3::ZERO, Vec3::X, Vec3::NAN, max),
        None
    );
    assert!(
        !GiReflectorBox {
            min,
            max,
            albedo: Vec3::ONE
        }
        .overlaps_sphere(Vec3::ZERO, -1.0)
    );
}

#[test]
fn raycast_returns_the_nearest_hit_and_validates_ranges() {
    let near = GiReflectorBox::from_center_extents(Vec3::new(2.0, 0.0, 0.0), Vec3::ONE, Vec3::X);
    let far = GiReflectorBox::from_center_extents(Vec3::new(5.0, 0.0, 0.0), Vec3::ONE, Vec3::Y);
    let reflectors = [far, near];
    let hit = raycast_all_reflectors(Vec3::ZERO, Vec3::X, 10.0, 0.0, &reflectors);
    assert_eq!(hit, Some((1.0, Vec3::NEG_X, Vec3::X)));
    assert_eq!(
        raycast_all_reflectors(Vec3::ZERO, Vec3::X, 0.0, 0.0, &reflectors),
        None
    );
    assert_eq!(
        raycast_all_reflectors(Vec3::ZERO, Vec3::X, -1.0, 0.0, &reflectors),
        None
    );
    assert_eq!(
        raycast_all_reflectors(Vec3::ZERO, Vec3::X, f32::NAN, 0.0, &reflectors),
        None
    );
}

#[test]
fn sphere_broadphase_preserves_full_raycast_results() {
    let reflectors = [
        GiReflectorBox::from_center_extents(Vec3::new(2.0, 0.0, 0.0), Vec3::ONE, Vec3::X),
        GiReflectorBox::from_center_extents(Vec3::new(-3.0, 0.0, 0.0), Vec3::ONE, Vec3::Y),
        GiReflectorBox::from_center_extents(Vec3::new(0.0, 4.0, 0.0), Vec3::ONE, Vec3::Z),
        GiReflectorBox::from_center_extents(Vec3::new(20.0, 0.0, 0.0), Vec3::ONE, Vec3::ONE),
    ];
    let filtered: Vec<_> = reflectors
        .iter()
        .filter(|r| r.overlaps_sphere(Vec3::ZERO, 6.0))
        .copied()
        .collect();
    let dirs = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::ONE.normalize(),
    ];
    for dir in dirs {
        let full = raycast_all_reflectors(Vec3::ZERO, dir, 6.0, 0.0, &reflectors);
        let pruned = raycast_all_reflectors(Vec3::ZERO, dir, 6.0, 0.0, &filtered);
        assert_eq!(full, pruned);
    }
}

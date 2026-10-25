use super::*;

fn triangle(vertices: [Vec3; 3], cull_backfaces: bool) -> GiTriangle {
    let Some(triangle) = GiTriangle::new(vertices, cull_backfaces) else {
        return GiTriangle {
            vertices,
            normal: Vec3::Y,
            cull_backfaces,
        };
    };
    triangle
}

fn floor(y: f32, reversed: bool, cull_backfaces: bool) -> GiTriangle {
    let mut vertices = [
        Vec3::new(-1.0, y, -1.0),
        Vec3::new(-1.0, y, 1.0),
        Vec3::new(1.0, y, -1.0),
    ];
    if reversed {
        vertices.reverse();
    }
    triangle(vertices, cull_backfaces)
}

fn bvh(triangles: &[GiTriangle]) -> GiTriangleBvh {
    let mut bvh = GiTriangleBvh::default();
    bvh.rebuild(&mut triangles.to_vec());
    bvh
}

fn downward_ray() -> GiRayQuery {
    GiRayQuery::new(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y, 0.0, 20.0)
}

#[test]
fn nearest_hit_uses_distance_and_geometric_normal() {
    let bvh = bvh(&[
        floor(0.0, false, true),
        floor(3.0, false, true),
        floor(1.0, false, true),
    ]);
    let hit = bvh.nearest_hit(downward_ray());
    assert_eq!(
        hit.map(|hit| (hit.distance, hit.normal)),
        Some((2.0, Vec3::Y))
    );
}

#[test]
fn default_culling_rejects_backfaces_and_opt_out_returns_winding_normal() {
    let culled = bvh(&[floor(0.0, true, true)]);
    assert_eq!(culled.nearest_hit(downward_ray()), None);

    let two_sided = bvh(&[floor(0.0, true, false)]);
    let hit = two_sided.nearest_hit(downward_ray());
    assert_eq!(
        hit.map(|hit| (hit.distance, hit.normal)),
        Some((5.0, Vec3::NEG_Y))
    );
}

#[test]
fn hit_sequence_tracks_nearest_distance_across_leaves() {
    let triangles: Vec<_> = (0..12)
        .map(|index| floor(index as f32 * 0.2, false, true))
        .collect();
    let bvh = bvh(&triangles);
    let distances: Vec<_> = (0..8)
        .map(|index| {
            let min_distance = index as f32 * 0.5;
            let ray = GiRayQuery {
                min_distance,
                ..downward_ray()
            };
            bvh.nearest_hit(ray).map(|hit| hit.distance)
        })
        .collect();
    assert_eq!(
        distances,
        vec![
            Some(2.8),
            Some(2.8),
            Some(2.8),
            Some(2.8),
            Some(2.8),
            Some(2.8),
            Some(3.0),
            Some(3.6),
        ]
    );
}

#[test]
fn degenerate_triangles_and_invalid_rays_do_not_hit() {
    let degenerate = GiTriangle::new([Vec3::ZERO; 3], true);
    assert_eq!(degenerate, None);
    let bvh = bvh(&[floor(0.0, false, false)]);
    let ray = GiRayQuery {
        direction: Vec3::ZERO,
        ..downward_ray()
    };
    assert_eq!(bvh.nearest_hit(ray), None);
}

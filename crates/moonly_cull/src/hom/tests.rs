use super::*;
use crate::hom_geometry::HomTriangle;

#[test]
fn rasterized_wall_hides_only_geometry_behind_it() {
    let mut view = MoonlyHomView {
        clip_from_world: moonly_core::perspective_rh_mat4(
            std::f32::consts::FRAC_PI_2,
            1.0,
            0.1,
            100.0,
        ),
        enabled: true,
        ..MoonlyHomView::default()
    };
    for vertices in [
        [
            Vec3::new(-2.0, -2.0, -5.0),
            Vec3::new(2.0, -2.0, -5.0),
            Vec3::new(-2.0, 2.0, -5.0),
        ],
        [
            Vec3::new(2.0, -2.0, -5.0),
            Vec3::new(2.0, 2.0, -5.0),
            Vec3::new(-2.0, 2.0, -5.0),
        ],
    ] {
        rasterize_triangle(
            &mut view,
            &HomTriangle {
                vertices,
                double_sided: false,
            },
        );
    }
    view.rasterizer.propagate();
    assert!(!view.aabb_visible(test_aabb(Vec3::new(0.0, 0.0, -10.0))));
    assert!(view.aabb_visible(test_aabb(Vec3::new(0.0, 0.0, -1.0))));
    assert!(view.aabb_visible(test_aabb(Vec3::new(20.0, 0.0, -10.0))));
}

#[test]
fn multiple_views_share_one_clock_tick_and_triangle_schedules() {
    let mut app = App::new();
    app.init_resource::<MoonlyConfig>()
        .init_resource::<MoonlyHomCacheConfig>()
        .init_resource::<MoonlyHomGeometry>()
        .init_resource::<HomFrameClock>()
        .init_resource::<HomTriangleSkipCache>()
        .init_resource::<HomVisibilityCache>()
        .init_resource::<MoonlyHomDiagnostics>()
        .add_systems(Update, update_hom_views);
    app.world_mut()
        .resource_mut::<MoonlyHomGeometry>()
        .triangles
        .push(HomTriangle {
            vertices: [
                Vec3::new(-2.0, -2.0, 5.0),
                Vec3::new(2.0, -2.0, 5.0),
                Vec3::new(-2.0, 2.0, 5.0),
            ],
            double_sided: false,
        });
    for _ in 0..2 {
        app.world_mut().spawn((
            MoonlyCamera::default(),
            Camera::default(),
            GlobalTransform::default(),
        ));
    }
    app.update();
    assert_eq!(app.world().resource::<HomFrameClock>().0, 1);
    app.update();
    assert_eq!(app.world().resource::<HomFrameClock>().0, 2);
    let diagnostics = app.world().resource::<MoonlyHomDiagnostics>();
    assert_eq!(diagnostics.skipped, 2);
}

fn test_aabb(center: Vec3) -> Aabb {
    Aabb::from_min_max(center - Vec3::splat(0.25), center + Vec3::splat(0.25))
}

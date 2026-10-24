//! Software hierarchical occlusion rasterizer and light visibility culling for Moonly.

pub mod light_vis;
pub mod occ_rasterizer;
pub mod smap_vis;

use bevy::prelude::*;

pub use crate::{
    light_vis::{
        EYE_IN_CONE_BIAS_RAD, EYE_IN_CONE_SCALE, LightVisibilityState, SAFE_WORLD_BOUND_METERS,
    },
    occ_rasterizer::{
        OCC_DEPTH_FAR, OCC_DIM_0, OCC_DIM_1, OCC_DIM_2, OCC_DIM_3, OccRasterizer, OccTestRect,
        df_2_s32,
    },
    smap_vis::{SMAP_VIS_SLEEP_FRAMES, SmapVisPhase, SmapVisTracker},
};

/// Plugin that registers software hierarchical occlusion culling and light visibility state.
#[derive(Debug, Default, Clone, Copy)]
pub struct MoonlyCullPlugin;

impl Plugin for MoonlyCullPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<OccRasterizer>()
            .register_type::<SmapVisPhase>()
            .register_type::<SmapVisTracker>()
            .register_type::<LightVisibilityState>()
            .init_resource::<OccRasterizer>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moonly_core::{XRayLight, XRayLightType};

    #[test]
    fn occ_rasterizer_occludes_behind_rasterized_quad_in_both_modes() {
        let mut occ = OccRasterizer::default();
        occ.clear();
        occ.rasterize_triangle(
            Vec3::new(10.0, 10.0, 0.25),
            Vec3::new(30.0, 10.0, 0.25),
            Vec3::new(10.0, 30.0, 0.25),
        );
        occ.rasterize_triangle(
            Vec3::new(30.0, 10.0, 0.25),
            Vec3::new(30.0, 30.0, 0.25),
            Vec3::new(10.0, 30.0, 0.25),
        );
        occ.propagate();

        let behind = OccTestRect {
            x0: 14,
            y0: 14,
            x1: 24,
            y1: 24,
            depth: 0.6,
        };
        assert_eq!(occ.test_monolith(behind), -1);
        assert_eq!(occ.test_enhanced_fast(behind), -1);

        let in_front = OccTestRect {
            x0: 14,
            y0: 14,
            x1: 24,
            y1: 24,
            depth: 0.1,
        };
        assert_eq!(occ.test_monolith(in_front), 1);
        assert_eq!(occ.test_enhanced_fast(in_front), 1);
    }

    #[test]
    fn smap_vis_tracks_invisible_casters_and_resets_cleanly() {
        let mut tracker = SmapVisTracker {
            candidates: vec![10, 20],
            ..SmapVisTracker::default()
        };
        tracker.step_frame(1);
        assert_eq!(tracker.phase, SmapVisPhase::Sleep);
        tracker.step_frame(1 + SMAP_VIS_SLEEP_FRAMES);
        assert_eq!(tracker.phase, SmapVisPhase::Test);
        tracker.last_probe_fragments = 0;
        tracker.step_frame(2 + SMAP_VIS_SLEEP_FRAMES);
        assert!(tracker.is_caster_invisible(10));
    }

    #[test]
    fn light_vis_skips_occlusion_query_when_eye_inside_spot_cone() {
        let light = XRayLight {
            light_type: XRayLightType::Spot,
            position: Vec3::ZERO,
            direction: Vec3::Z,
            range: 10.0,
            cone_angle: 1.2,
            ..XRayLight::default()
        };
        let mut vis = LightVisibilityState::default();
        assert!(!vis.vis_prepare(&light, Vec3::new(0.0, 0.0, 2.0), 5));
        assert!(vis.visible);
    }

    #[test]
    fn occlusion_queries_use_strict_depth_equality() {
        let mut occ = OccRasterizer::default();
        occ.rasterize_triangle(
            Vec3::new(0.0, 0.0, 0.5),
            Vec3::new(64.0, 0.0, 0.5),
            Vec3::new(0.0, 64.0, 0.5),
        );
        occ.rasterize_triangle(
            Vec3::new(64.0, 0.0, 0.5),
            Vec3::new(64.0, 64.0, 0.5),
            Vec3::new(0.0, 64.0, 0.5),
        );
        occ.propagate();
        let query = OccTestRect {
            x0: 20,
            y0: 20,
            x1: 30,
            y1: 30,
            depth: 0.5,
        };
        assert_eq!(occ.test_monolith(query), -1);
        assert_eq!(occ.test_enhanced_fast(query), -1);
    }

    #[test]
    fn hierarchical_queries_match_full_resolution_reference() {
        let mut occ = OccRasterizer::default();
        occ.rasterize_triangle(
            Vec3::new(3.0, 5.0, 0.2),
            Vec3::new(55.0, 9.0, 0.5),
            Vec3::new(13.0, 58.0, 0.8),
        );
        occ.rasterize_triangle(
            Vec3::new(55.0, 9.0, 0.5),
            Vec3::new(60.0, 50.0, 0.3),
            Vec3::new(13.0, 58.0, 0.8),
        );
        occ.propagate();
        for index in 0..80_usize {
            let x0 = (index * 17) % 64;
            let y0 = (index * 29) % 64;
            let query = OccTestRect {
                x0,
                y0,
                x1: (x0 + index % 13).min(63),
                y1: (y0 + index % 7).min(63),
                depth: index as f32 / 79.0,
            };
            let expected = reference_query(&occ, query);
            assert_eq!(occ.test_monolith(query), expected, "Monolith query {index}");
            assert_eq!(
                occ.test_enhanced_fast(query),
                expected,
                "fast query {index}"
            );
        }
    }

    #[test]
    fn reversed_triangle_winding_preserves_coverage_and_depth() {
        let vertices = [
            Vec3::new(8.0, 8.0, 0.25),
            Vec3::new(40.0, 12.0, 0.5),
            Vec3::new(16.0, 45.0, 0.75),
        ];
        let [v0, v1, v2] = vertices;
        let mut first = OccRasterizer::default();
        let mut second = OccRasterizer::default();
        first.rasterize_triangle(v0, v1, v2);
        second.rasterize_triangle(v2, v1, v0);
        let (coverage, max_delta) = first.buf_depth_0.iter().zip(&second.buf_depth_0).fold(
            (0, 0_i32),
            |(coverage, max_delta), (a, b)| {
                let delta = a.abs_diff(*b).min(i32::MAX as u32) as i32;
                (
                    coverage + usize::from((a == &OCC_DEPTH_FAR) != (b == &OCC_DEPTH_FAR)),
                    max_delta.max(delta),
                )
            },
        );
        assert_eq!(coverage, 0);
        assert!(max_delta <= 256, "fixed-point depth delta {max_delta}");
    }

    fn reference_query(occ: &OccRasterizer, rect: OccTestRect) -> i32 {
        let depth = df_2_s32(rect.depth);
        let x0 = rect.x0.min(OCC_DIM_0 - 1);
        let y0 = rect.y0.min(OCC_DIM_0 - 1);
        let x1 = rect.x1.min(OCC_DIM_0 - 1).max(x0);
        let y1 = rect.y1.min(OCC_DIM_0 - 1).max(y0);
        for y in y0..=y1 {
            for x in x0..=x1 {
                if occ
                    .buf_depth_0
                    .get(y * OCC_DIM_0 + x)
                    .is_some_and(|&stored| depth < stored)
                {
                    return 1;
                }
            }
        }
        -1
    }
}

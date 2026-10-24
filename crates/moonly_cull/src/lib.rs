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
}

use bevy::{prelude::*, render::view::ExtractedView};
use moonly_core::{MoonlyLight, MoonlyLightType};

#[derive(Clone, Copy)]
struct VisibilityBasis {
    position: Vec3,
    right: Vec3,
    up: Vec3,
    forward: Vec3,
    tangent: Vec2,
}

pub(crate) fn light_visible(light: &MoonlyLight, view: &ExtractedView) -> bool {
    if light.light_type == MoonlyLightType::Direct {
        return true;
    }
    let projection = view.clip_from_view;
    let basis = VisibilityBasis {
        position: view.world_from_view.translation(),
        right: view.world_from_view.right().as_vec3(),
        up: view.world_from_view.up().as_vec3(),
        forward: view.world_from_view.forward().as_vec3(),
        tangent: Vec2::new(
            1.0 / projection.x_axis.x.abs().max(1.0e-6),
            1.0 / projection.y_axis.y.abs().max(1.0e-6),
        ),
    };
    sphere_visible(light.position, light.range, basis)
}

fn sphere_visible(position: Vec3, range: f32, basis: VisibilityBasis) -> bool {
    let offset = position - basis.position;
    let depth = offset.dot(basis.forward);
    if depth < -range {
        return false;
    }
    let expanded_depth = depth.max(0.0);
    let lateral_x = range + expanded_depth * basis.tangent.x;
    let lateral_y = range + expanded_depth * basis.tangent.y;
    offset.dot(basis.right).abs() <= lateral_x && offset.dot(basis.up).abs() <= lateral_y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frustum_fallback_accepts_boundary_and_rejects_hidden_lights() {
        let basis = VisibilityBasis {
            position: Vec3::ZERO,
            right: Vec3::X,
            up: Vec3::Y,
            forward: Vec3::NEG_Z,
            tangent: Vec2::splat(1.0),
        };
        assert!(sphere_visible(Vec3::new(5.0, 0.0, -5.0), 1.0, basis));
        assert!(sphere_visible(Vec3::new(0.0, 0.0, 0.5), 1.0, basis));
        assert!(!sphere_visible(Vec3::new(0.0, 0.0, 5.0), 1.0, basis));
        assert!(!sphere_visible(Vec3::new(20.0, 0.0, -5.0), 1.0, basis));
    }
}

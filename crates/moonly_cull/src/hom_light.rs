use bevy::{camera::primitives::Aabb, prelude::*};
use moonly_core::{LightPackage, MoonlyConfig, MoonlyLight, MoonlyLightType};
use moonly_render::{MoonlyCamera, MoonlyCameraRole};

use crate::hom::MoonlyHomView;

pub(crate) fn apply_hom_light_visibility(
    config: Res<MoonlyConfig>,
    cameras: Query<(&MoonlyHomView, &MoonlyCamera)>,
    mut package: ResMut<LightPackage>,
) {
    if !config.enable_occ_rasterizer {
        return;
    }
    let Some((hom, _)) = cameras
        .iter()
        .find(|(hom, camera)| hom.enabled && camera.role == MoonlyCameraRole::World)
    else {
        return;
    };
    package.v_point.retain(|light| light_visible(hom, light));
    package.v_spot.retain(|light| light_visible(hom, light));
    package.v_shadowed.retain(|light| light_visible(hom, light));
}

fn light_visible(hom: &MoonlyHomView, light: &MoonlyLight) -> bool {
    if light.light_type == MoonlyLightType::Direct {
        return true;
    }
    let extent = Vec3::splat(light.range.max(0.0));
    hom.aabb_visible(Aabb::from_min_max(
        light.position - extent,
        light.position + extent,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directional_lights_bypass_hom() {
        let hom = MoonlyHomView::default();
        let light = MoonlyLight {
            light_type: MoonlyLightType::Direct,
            ..MoonlyLight::default()
        };
        assert!(light_visible(&hom, &light));
    }
}

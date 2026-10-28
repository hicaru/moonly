use bevy::{camera::primitives::Aabb, prelude::*};
use moonly_core::{LightPackage, MoonlyConfig, MoonlyLight, MoonlyLightType};
use moonly_render::{MoonlyCamera, MoonlyCameraRole};

use crate::{hom::MoonlyHomView, hom_cache::HomVisibilityCache};

pub(crate) fn apply_hom_light_visibility(
    config: Res<MoonlyConfig>,
    cameras: Query<(&MoonlyHomView, &MoonlyCamera)>,
    mut package: ResMut<LightPackage>,
    mut cache: ResMut<HomVisibilityCache>,
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
    package
        .v_point
        .retain(|light| light_visible(hom, &mut cache, light));
    package
        .v_spot
        .retain(|light| light_visible(hom, &mut cache, light));
    package
        .v_shadowed
        .retain(|light| light_visible(hom, &mut cache, light));
}

fn light_visible(hom: &MoonlyHomView, cache: &mut HomVisibilityCache, light: &MoonlyLight) -> bool {
    if light.light_type == MoonlyLightType::Direct {
        return true;
    }
    let extent = Vec3::splat(light.range.max(0.0));
    hom.aabb_visible_cached(
        cache,
        light_cache_key(light),
        Aabb::from_min_max(light.position - extent, light.position + extent),
    )
}

fn light_cache_key(light: &MoonlyLight) -> u64 {
    let mut key = light.source_entity.map_or_else(
        || {
            let mut key = 0xcbf2_9ce4_8422_2325_u64;
            for value in [
                light.position.x.to_bits(),
                light.position.y.to_bits(),
                light.position.z.to_bits(),
                light.range.to_bits(),
                light.light_type as u32,
            ] {
                key ^= u64::from(value);
                key = key.wrapping_mul(0x1000_0000_01b3);
            }
            key
        },
        Entity::to_bits,
    );
    if let Some(face) = light.omnipart_face {
        key ^= 0x9e37_79b9_7f4a_7c15_u64.wrapping_mul(u64::from(face) + 1);
    }
    key
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
        assert!(light_visible(
            &hom,
            &mut HomVisibilityCache::default(),
            &light
        ));
    }
}

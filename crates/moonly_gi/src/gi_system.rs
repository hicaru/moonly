use bevy::prelude::*;
use moonly_core::{LightPackage, MoonlyConfig, MoonlyPipelineTier, XRayLight, XRayLightType};

use crate::gi_bounce::{GiLightField, GiReflectorBox, gi_generate_enhanced, gi_generate_monolith};

const OMNIPART_FACE_COUNT: usize = 6;

/// Update the virtual light field from every eligible light package bucket.
pub fn update_gi_light_field(
    config: Res<MoonlyConfig>,
    package: Res<LightPackage>,
    reflector_query: Query<&GiReflectorBox>,
    mut field: ResMut<GiLightField>,
    mut buffers: Local<(Vec<GiReflectorBox>, Vec<GiReflectorBox>)>,
) {
    let (ref mut reflectors, ref mut scratch) = *buffers;
    reflectors.clear();
    reflectors.extend(reflector_query.iter().copied());
    generate_package_gi(&config, &package, reflectors, &mut field, scratch);
}

pub(crate) fn generate_package_gi(
    config: &MoonlyConfig,
    package: &LightPackage,
    reflectors: &[GiReflectorBox],
    field: &mut GiLightField,
    scratch: &mut Vec<GiReflectorBox>,
) {
    field.virtual_lights.clear();
    generate_shadowed_gi(config, &package.v_shadowed, reflectors, field, scratch);
    for light in package.v_spot.iter().chain(&package.v_point) {
        generate_light_gi(config, light, reflectors, field, scratch);
    }
}

fn generate_shadowed_gi(
    config: &MoonlyConfig,
    lights: &[XRayLight],
    reflectors: &[GiReflectorBox],
    field: &mut GiLightField,
    scratch: &mut Vec<GiReflectorBox>,
) {
    let mut index = 0_usize;
    while let Some(light) = lights.get(index) {
        let group_len = omnipart_group_len(lights, index);
        if group_len == OMNIPART_FACE_COUNT {
            let mut point = light.clone();
            point.light_type = XRayLightType::Point;
            generate_light_gi(config, &point, reflectors, field, scratch);
            index += group_len;
        } else {
            generate_light_gi(config, light, reflectors, field, scratch);
            index += 1;
        }
    }
}

fn omnipart_group_len(lights: &[XRayLight], first: usize) -> usize {
    let Some(light) = lights.get(first) else {
        return 0;
    };
    if !matches!(light.light_type, XRayLightType::Omnipart) {
        return 1;
    }
    lights.get(first..).map_or(1, |tail| {
        tail.iter()
            .take(OMNIPART_FACE_COUNT)
            .take_while(|part| same_omnipart_source(light, part))
            .count()
    })
}

fn same_omnipart_source(light: &XRayLight, part: &XRayLight) -> bool {
    matches!(part.light_type, XRayLightType::Omnipart)
        && light.position == part.position
        && light.color == part.color
        && light.range.to_bits() == part.range.to_bits()
        && light.flags == part.flags
        && light.smap_size_override == part.smap_size_override
        && light.volumetric == part.volumetric
}

fn generate_light_gi(
    config: &MoonlyConfig,
    light: &XRayLight,
    reflectors: &[GiReflectorBox],
    field: &mut GiLightField,
    scratch: &mut Vec<GiReflectorBox>,
) {
    if matches!(config.tier, MoonlyPipelineTier::MoonlyEnhanced) {
        gi_generate_enhanced(
            light,
            &config.gi,
            reflectors,
            &mut field.virtual_lights,
            scratch,
        );
    } else {
        gi_generate_monolith(light, &config.gi, reflectors, &mut field.virtual_lights);
    }
}

#[cfg(test)]
mod tests;

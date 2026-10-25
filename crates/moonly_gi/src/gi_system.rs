use bevy::{ecs::system::SystemParam, prelude::*};
#[cfg(test)]
use moonly_core::{LightPackage, MoonlyLightType, MoonlyPipelineTier};
use moonly_core::{MoonlyConfig, MoonlyLight, VirtualGiLightPackage};

use crate::gi_bounce::{GiLightField, GiReflectorBox};
#[cfg(test)]
use crate::gi_bounce::{gi_generate_enhanced, gi_generate_monolith};
use crate::gi_cache::{GiGenerationContext, GiLightCache, generate_cached_light};
use crate::gi_geometry::StaticGiGeometry;

#[cfg(test)]
const OMNIPART_FACE_COUNT: usize = 6;

#[derive(SystemParam)]
pub struct GiSceneInputs<'w, 's> {
    config: Res<'w, MoonlyConfig>,
    lights: Query<'w, 's, (Entity, &'static MoonlyLight)>,
    changed_lights: Query<'w, 's, (), Changed<MoonlyLight>>,
    reflectors: Query<'w, 's, &'static GiReflectorBox>,
    changed_reflectors: Query<'w, 's, (), Changed<GiReflectorBox>>,
    geometry: Res<'w, StaticGiGeometry>,
}

#[derive(Default)]
pub struct GiFrameBuffers {
    reflectors: Vec<GiReflectorBox>,
    scratch: Vec<GiReflectorBox>,
    last_revision: u64,
}

/// Update cached virtual lights from stable source light entities.
pub fn update_gi_light_field(
    inputs: GiSceneInputs,
    mut field: ResMut<GiLightField>,
    mut cache: ResMut<GiLightCache>,
    mut package: ResMut<VirtualGiLightPackage>,
    mut buffers: Local<GiFrameBuffers>,
) {
    let geo_rev = inputs.geometry.revision();
    if !inputs.config.is_changed()
        && geo_rev == buffers.last_revision
        && inputs.changed_lights.is_empty()
        && inputs.changed_reflectors.is_empty()
        && inputs.lights.iter().count() == cache.light_count()
        && inputs.reflectors.iter().count() == buffers.reflectors.len()
        && field.virtual_lights.len() == package.lights.len()
    {
        return;
    }
    buffers.last_revision = geo_rev;
    field.virtual_lights.clear();
    cache.retain(&inputs.lights);
    let GiFrameBuffers {
        reflectors,
        scratch,
        ..
    } = &mut *buffers;
    reflectors.clear();
    reflectors.extend(inputs.reflectors.iter().copied());
    let mut context = GiGenerationContext {
        config: &inputs.config,
        geometry: &inputs.geometry,
        reflectors,
        scratch,
    };
    for (entity, light) in &inputs.lights {
        generate_cached_light(&mut cache, entity, light, &mut context, &mut field);
    }
    package.lights.clear();
    package.lights.extend_from_slice(&field.virtual_lights);
}

#[cfg(test)]
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

#[cfg(test)]
fn generate_shadowed_gi(
    config: &MoonlyConfig,
    lights: &[MoonlyLight],
    reflectors: &[GiReflectorBox],
    field: &mut GiLightField,
    scratch: &mut Vec<GiReflectorBox>,
) {
    let mut index = 0_usize;
    while let Some(light) = lights.get(index) {
        let group_len = omnipart_group_len(lights, index);
        if group_len == OMNIPART_FACE_COUNT {
            let mut point = light.clone();
            point.light_type = MoonlyLightType::Point;
            generate_light_gi(config, &point, reflectors, field, scratch);
            index += group_len;
        } else {
            generate_light_gi(config, light, reflectors, field, scratch);
            index += 1;
        }
    }
}

#[cfg(test)]
fn omnipart_group_len(lights: &[MoonlyLight], first: usize) -> usize {
    let Some(light) = lights.get(first) else {
        return 0;
    };
    if !matches!(light.light_type, MoonlyLightType::Omnipart) {
        return 1;
    }
    lights.get(first..).map_or(1, |tail| {
        tail.iter()
            .take(OMNIPART_FACE_COUNT)
            .take_while(|part| same_omnipart_source(light, part))
            .count()
    })
}

#[cfg(test)]
fn same_omnipart_source(light: &MoonlyLight, part: &MoonlyLight) -> bool {
    matches!(part.light_type, MoonlyLightType::Omnipart)
        && light.position == part.position
        && light.color == part.color
        && light.range.to_bits() == part.range.to_bits()
        && light.flags == part.flags
        && light.smap_size_override == part.smap_size_override
        && light.volumetric == part.volumetric
}

#[cfg(test)]
fn generate_light_gi(
    config: &MoonlyConfig,
    light: &MoonlyLight,
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

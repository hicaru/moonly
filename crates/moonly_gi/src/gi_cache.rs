use bevy::{ecs::entity::EntityHashMap, prelude::*};
use moonly_core::{
    GiRadiosityConfig, MoonlyConfig, MoonlyLight, MoonlyLightFlags, MoonlyLightType,
    MoonlyPipelineTier, VirtualGiLight,
};

use crate::gi_bounce::{GiLightField, GiReflectorBox, gi_generate_enhanced, gi_generate_monolith};
use crate::gi_geometry::StaticGiGeometry;
use crate::gi_ray::GiCombinedGeometry;

#[derive(Debug, Clone, PartialEq)]
struct GiLightKey {
    light_type: MoonlyLightType,
    gi_enabled: bool,
    color: Vec3,
    position: Vec3,
    direction: Vec3,
    right: Vec3,
    range: f32,
    cone_angle: f32,
    config: GiRadiosityConfig,
    tier: MoonlyPipelineTier,
    geometry_revision: u64,
}

impl GiLightKey {
    const fn new(light: &MoonlyLight, config: &MoonlyConfig, geometry_revision: u64) -> Self {
        Self {
            light_type: light.light_type,
            gi_enabled: light.flags.contains(MoonlyLightFlags::GI_ENABLED),
            color: light.color,
            position: light.position,
            direction: light.direction,
            right: light.right,
            range: light.range,
            cone_angle: light.cone_angle,
            config: config.gi,
            tier: config.tier,
            geometry_revision,
        }
    }
}

#[derive(Debug, Default)]
struct CachedLight {
    key: Option<GiLightKey>,
    virtual_lights: Vec<VirtualGiLight>,
}

/// Per-entity virtual lights retained across unchanged frames.
#[derive(Resource, Debug, Default)]
pub struct GiLightCache {
    entries: EntityHashMap<CachedLight>,
}

impl GiLightCache {
    /// Return the number of cached source lights.
    #[must_use]
    pub fn light_count(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn retain(&mut self, lights: &Query<(Entity, &MoonlyLight)>) {
        self.entries.retain(|entity, _| lights.get(*entity).is_ok());
    }
}

pub(crate) struct GiGenerationContext<'a> {
    pub(crate) config: &'a MoonlyConfig,
    pub(crate) geometry: &'a StaticGiGeometry,
    pub(crate) reflectors: &'a [GiReflectorBox],
    pub(crate) scratch: &'a mut Vec<GiReflectorBox>,
}

impl GiGenerationContext<'_> {
    fn generate(&mut self, light: &MoonlyLight, out: &mut Vec<VirtualGiLight>) {
        if matches!(self.config.tier, MoonlyPipelineTier::MoonlyEnhanced) {
            gi_generate_enhanced(light, &self.config.gi, self.reflectors, out, self.scratch);
        } else {
            let geometry = GiCombinedGeometry {
                triangles: self.geometry,
                reflectors: self.reflectors,
            };
            gi_generate_monolith(light, &self.config.gi, &geometry, out);
        }
    }
}

pub(crate) fn generate_cached_light(
    cache: &mut GiLightCache,
    entity: Entity,
    light: &MoonlyLight,
    context: &mut GiGenerationContext,
    field: &mut GiLightField,
) {
    let key = GiLightKey::new(light, context.config, context.geometry.revision());
    let cached = cache.entries.entry(entity).or_default();
    if cached.key.as_ref() != Some(&key) {
        cached.virtual_lights.clear();
        context.generate(light, &mut cached.virtual_lights);
        cached.key = Some(key);
    }
    field
        .virtual_lights
        .extend_from_slice(&cached.virtual_lights);
}

#[cfg(test)]
mod tests;

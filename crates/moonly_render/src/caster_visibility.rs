use std::collections::HashSet;

use bevy::{
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
};

/// Shared light and caster visibility for shadow draws.
#[derive(Resource, Debug, Default, Clone)]
pub struct MoonlyCasterVisibility {
    invisible: HashSet<(Entity, Entity)>,
}

impl MoonlyCasterVisibility {
    /// Clear all cached pairs and keep the allocated storage.
    pub fn clear(&mut self) {
        self.invisible.clear();
    }

    /// Mark one caster as invisible for one light.
    pub fn insert(&mut self, light: Entity, caster: Entity) {
        self.invisible.insert((light, caster));
    }

    /// Check one light and caster pair.
    #[must_use]
    pub fn is_caster_invisible(&self, light: Entity, caster: Entity) -> bool {
        self.invisible.contains(&(light, caster))
    }
}

impl ExtractResource<RenderApp> for MoonlyCasterVisibility {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

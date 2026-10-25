use bevy::{
    ecs::query::QueryItem,
    prelude::*,
    render::{RenderApp, extract_component::ExtractComponent, sync_component::SyncComponent},
};

use crate::MoonlyLight;

impl ExtractComponent<RenderApp> for MoonlyProjector {
    type QueryData = &'static Self;
    type QueryFilter = With<MoonlyLight>;
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}

/// Projective texture for an `MoonlyLight` spot source.
#[derive(Component, Debug, Clone, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub struct MoonlyProjector {
    /// Texture sampled with the light projection matrix.
    pub texture: Handle<Image>,
}

impl SyncComponent<RenderApp> for MoonlyProjector {
    type Target = Self;
}

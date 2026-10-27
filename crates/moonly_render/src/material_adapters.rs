use std::{any::TypeId, collections::HashMap, sync::Arc};

use bevy::{
    asset::Handle,
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
    shader::Shader,
};

/// G-buffer shaders registered for material types that extend `StandardMaterial`.
#[derive(Resource, Default, Clone)]
pub struct MoonlyMaterialAdapters(Arc<HashMap<TypeId, Handle<Shader>>>);

impl MoonlyMaterialAdapters {
    pub fn set<M: 'static>(&mut self, shader: Handle<Shader>) {
        Arc::make_mut(&mut self.0).insert(TypeId::of::<M>(), shader);
    }

    #[must_use]
    pub fn get(&self, material: TypeId) -> Option<&Handle<Shader>> {
        self.0.get(&material)
    }
}

impl ExtractResource<RenderApp> for MoonlyMaterialAdapters {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

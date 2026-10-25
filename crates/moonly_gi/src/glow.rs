use std::collections::HashMap;

use bevy::{
    asset::{AssetEvent, AssetId},
    camera::visibility::RenderLayers,
    ecs::system::SystemParam,
    mesh::skinning::SkinnedMesh,
    prelude::*,
};
use moonly_core::{MoonlyLight, MoonlyLightFlags, MoonlyLightType};

use crate::{GiTriangle, gi_geometry::append_mesh_triangles};

const MAX_GLOW_SAMPLES: u16 = 64;
const GLOW_SURFACE_OFFSET: f32 = 0.02;
const FULL_SPHERE_STERADIANS: f32 = 4.0 * std::f32::consts::PI;

/// Convert a static mesh emitter into a bounded set of virtual point lights.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct MoonlyGlow {
    /// Linear RGB emitter color without intensity.
    pub color: Vec3,
    /// Total luminous power distributed across all samples.
    pub lumens: f32,
    /// Number of area-weighted samples in `1..=64`.
    pub samples: u16,
    /// Maximum point light attenuation radius in meters.
    pub range: f32,
}

impl Default for MoonlyGlow {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            lumens: 1000.0,
            samples: 4,
            range: 8.0,
        }
    }
}

/// Cached light entities and change counter for mesh emitter sampling.
#[derive(Resource, Debug, Default)]
pub struct MoonlyGlowCache {
    entries: HashMap<Entity, Vec<Entity>>,
    revision: u64,
}

impl MoonlyGlowCache {
    /// Return the number of active virtual point lights.
    #[must_use]
    pub fn light_count(&self) -> usize {
        self.entries.values().map(Vec::len).sum()
    }

    /// Return the number of completed emitter samples.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

type GlowComponents = (
    Entity,
    Ref<'static, Mesh3d>,
    Ref<'static, GlobalTransform>,
    Ref<'static, MoonlyGlow>,
    Option<&'static InheritedVisibility>,
    Option<&'static RenderLayers>,
);
type GlowQuery<'w, 's> = Query<'w, 's, GlowComponents, Without<SkinnedMesh>>;

#[derive(SystemParam)]
pub(crate) struct GlowSyncInputs<'w, 's> {
    meshes: Option<Res<'w, Assets<Mesh>>>,
    events: MessageReader<'w, 's, AssetEvent<Mesh>>,
    emitters: GlowQuery<'w, 's>,
    removed: RemovedComponents<'w, 's, MoonlyGlow>,
}

#[derive(SystemParam)]
pub(crate) struct GlowScratch<'s> {
    triangles: Local<'s, Vec<GiTriangle>>,
    changed_assets: Local<'s, Vec<AssetId<Mesh>>>,
    stale: Local<'s, Vec<Entity>>,
}

pub(crate) fn sync_mesh_emitters(
    mut commands: Commands,
    mut cache: ResMut<MoonlyGlowCache>,
    mut inputs: GlowSyncInputs,
    mut scratch: GlowScratch,
) {
    scratch.changed_assets.clear();
    for event in inputs.events.read() {
        match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::Removed { id } => {
                scratch.changed_assets.push(*id);
            }
            _ => {}
        }
    }
    remove_stale_emitters(&mut commands, &mut cache, &mut inputs, &mut scratch.stale);
    for (entity, mesh, transform, glow, visibility, layers) in &inputs.emitters {
        let hidden = visibility.is_some_and(|visibility| !visibility.get());
        let dirty = scratch.changed_assets.contains(&mesh.id())
            || !cache.entries.contains_key(&entity)
            || mesh.is_changed()
            || transform.is_changed()
            || glow.is_changed();
        if dirty {
            resample_emitter(
                &mut commands,
                &mut cache,
                &inputs,
                &mut scratch.triangles,
                EmitterSample {
                    entity,
                    mesh: &mesh,
                    transform: &transform,
                    glow: &glow,
                    layers,
                    hidden,
                },
            );
        }
    }
}

struct EmitterSample<'a> {
    entity: Entity,
    mesh: &'a Mesh3d,
    transform: &'a GlobalTransform,
    glow: &'a MoonlyGlow,
    layers: Option<&'a RenderLayers>,
    hidden: bool,
}

fn resample_emitter(
    commands: &mut Commands,
    cache: &mut MoonlyGlowCache,
    inputs: &GlowSyncInputs,
    triangles: &mut Vec<GiTriangle>,
    sample: EmitterSample,
) {
    clear_emitter(commands, cache, sample.entity);
    triangles.clear();
    if !sample.hidden
        && let Some(mesh) = inputs
            .meshes
            .as_ref()
            .and_then(|meshes| meshes.get(sample.mesh))
    {
        append_mesh_triangles(mesh, sample.transform, false, triangles);
    }
    let lights = spawn_samples(commands, &sample, triangles);
    cache.entries.insert(sample.entity, lights);
    cache.revision = cache.revision.wrapping_add(1);
}

fn spawn_samples(
    commands: &mut Commands,
    sample: &EmitterSample,
    triangles: &[GiTriangle],
) -> Vec<Entity> {
    let count = sample.glow.samples.clamp(1, MAX_GLOW_SAMPLES);
    let total_area = triangles.iter().map(triangle_area).sum::<f32>();
    if total_area <= 1.0e-8 || sample.glow.lumens <= 0.0 {
        return Vec::new();
    }
    let intensity = sample.glow.lumens / (f32::from(count) * FULL_SPHERE_STERADIANS);
    (0..count)
        .filter_map(|index| {
            let fraction = (f32::from(index) + 0.5) / f32::from(count);
            let (position, normal) = sample_triangle(triangles, total_area * fraction)?;
            let mut light = MoonlyLight {
                light_type: MoonlyLightType::Point,
                flags: MoonlyLightFlags::GI_ENABLED,
                color: sample.glow.color * intensity,
                range: sample.glow.range,
                ..MoonlyLight::default()
            };
            if let Some(layers) = sample.layers {
                light.render_layers = layer_words(layers);
            }
            let mut entity = commands.spawn((
                light,
                Transform::from_translation(position).looking_at(position + normal, Vec3::Y),
            ));
            if let Some(layers) = sample.layers {
                entity.insert(layers.clone());
            }
            Some(entity.id())
        })
        .collect()
}

fn sample_triangle(triangles: &[GiTriangle], target_area: f32) -> Option<(Vec3, Vec3)> {
    let mut accumulated = 0.0;
    for triangle in triangles {
        accumulated += triangle_area(triangle);
        if accumulated >= target_area {
            let position = triangle.vertices.iter().copied().sum::<Vec3>() / 3.0;
            return Some((
                position + triangle.normal * GLOW_SURFACE_OFFSET,
                triangle.normal,
            ));
        }
    }
    None
}

fn triangle_area(triangle: &GiTriangle) -> f32 {
    let (Some(first), Some(second), Some(third)) = (
        triangle.vertices.first(),
        triangle.vertices.get(1),
        triangle.vertices.get(2),
    ) else {
        return 0.0;
    };
    (*second - *first).cross(*third - *first).length() * 0.5
}

fn layer_words(layers: &RenderLayers) -> [u64; 4] {
    let mut words = [0; 4];
    for (index, bits) in layers.bits().iter().enumerate() {
        if let Some(target) = words.get_mut(index) {
            *target = *bits;
        }
    }
    words
}

fn remove_stale_emitters(
    commands: &mut Commands,
    cache: &mut MoonlyGlowCache,
    inputs: &mut GlowSyncInputs,
    stale: &mut Vec<Entity>,
) {
    stale.clear();
    stale.extend(
        cache
            .entries
            .keys()
            .filter(|entity| inputs.emitters.get(**entity).is_err())
            .copied(),
    );
    stale.extend(inputs.removed.read());
    for entity in stale.drain(..) {
        clear_emitter(commands, cache, entity);
    }
}

fn clear_emitter(commands: &mut Commands, cache: &mut MoonlyGlowCache, entity: Entity) {
    if let Some(lights) = cache.entries.remove(&entity) {
        for light in lights {
            commands.entity(light).despawn();
        }
    }
}

#[cfg(test)]
mod tests;

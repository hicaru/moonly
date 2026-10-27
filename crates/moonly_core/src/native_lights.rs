use bevy::{
    light::PointLightShadowMap,
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
};

use crate::{MoonlyLight, MoonlyLightFlags, MoonlyLightType, WeatherEnvironment};

const FULL_SPHERE_STERADIANS: f32 = 4.0 * std::f32::consts::PI;

/// Enable native `GlobalAmbientLight` synchronization for Bevy applications.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct MoonlyNativeAmbient(pub bool);

impl ExtractResource<RenderApp> for MoonlyNativeAmbient {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

/// Enable native Bevy tone mapping and color grading.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct MoonlyNativePost(pub bool);

impl ExtractResource<RenderApp> for MoonlyNativePost {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

/// Use Bevy forward rendering for native lights.
/// Insert this resource before the plugin to disable Moonly-only subsystems.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
pub struct MoonlyNativeForward(pub bool);

impl ExtractResource<RenderApp> for MoonlyNativeForward {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct MoonlyNativeLight;

pub(crate) fn sync_native_ambient_light(
    ambient: Option<Res<GlobalAmbientLight>>,
    native: Res<MoonlyNativeAmbient>,
    forward: Res<MoonlyNativeForward>,
    mut environment: ResMut<WeatherEnvironment>,
) {
    if !native.0 || forward.0 {
        return;
    }
    let (color, brightness) =
        ambient.map_or((Color::BLACK, 0.0), |light| (light.color, light.brightness));
    let linear = color.to_linear();
    environment.ambient_color = Vec3::new(linear.red, linear.green, linear.blue) * brightness;
    environment.hemi_color = Vec4::ZERO;
}

pub(crate) fn remove_native_light_adapters(
    mut commands: Commands,
    mut points: RemovedComponents<PointLight>,
    mut spots: RemovedComponents<SpotLight>,
    mut directs: RemovedComponents<DirectionalLight>,
    managed: Query<(), With<MoonlyNativeLight>>,
) {
    let removed = points.read().chain(spots.read()).chain(directs.read());
    for entity in removed {
        if managed.contains(entity) {
            commands
                .entity(entity)
                .remove::<(MoonlyLight, MoonlyNativeLight)>();
        }
    }
}

pub(crate) fn sync_native_point_lights(
    mut commands: Commands,
    shadow_map: Res<PointLightShadowMap>,
    forward: Res<MoonlyNativeForward>,
    mut lights: Query<(
        Entity,
        &PointLight,
        Option<&mut MoonlyLight>,
        Has<MoonlyNativeLight>,
    )>,
) {
    if forward.0 {
        return;
    }
    for (entity, native, current, managed) in &mut lights {
        if current.is_some() && !managed {
            continue;
        }
        let adapted = adapt_point(native, &shadow_map);
        update_or_insert(&mut commands, entity, current, adapted);
    }
}

pub(crate) fn sync_native_spot_lights(
    mut commands: Commands,
    shadow_map: Res<PointLightShadowMap>,
    forward: Res<MoonlyNativeForward>,
    mut lights: Query<(
        Entity,
        &SpotLight,
        Option<&mut MoonlyLight>,
        Has<MoonlyNativeLight>,
    )>,
) {
    if forward.0 {
        return;
    }
    for (entity, native, current, managed) in &mut lights {
        if current.is_some() && !managed {
            continue;
        }
        let adapted = adapt_spot(native, &shadow_map);
        update_or_insert(&mut commands, entity, current, adapted);
    }
}

pub(crate) fn sync_native_directional_lights(
    mut commands: Commands,
    forward: Res<MoonlyNativeForward>,
    mut lights: Query<(
        Entity,
        &DirectionalLight,
        Option<&mut MoonlyLight>,
        Has<MoonlyNativeLight>,
    )>,
) {
    if forward.0 {
        return;
    }
    for (entity, native, current, managed) in &mut lights {
        if current.is_some() && !managed {
            continue;
        }
        let adapted = adapt_directional(native);
        update_or_insert(&mut commands, entity, current, adapted);
    }
}

fn update_or_insert(
    commands: &mut Commands,
    entity: Entity,
    current: Option<Mut<MoonlyLight>>,
    adapted: MoonlyLight,
) {
    if let Some(mut current) = current {
        if *current != adapted {
            *current = adapted;
        }
        return;
    }
    commands.entity(entity).insert((adapted, MoonlyNativeLight));
}

fn adapt_point(light: &PointLight, shadow_map: &PointLightShadowMap) -> MoonlyLight {
    MoonlyLight {
        light_type: MoonlyLightType::Point,
        flags: native_flags(light.shadow_maps_enabled),
        color: linear_color(light.color, light.intensity / FULL_SPHERE_STERADIANS),
        range: light.range,
        shadow_near_z: light.shadow_map_near_z,
        smap_size_override: shadow_map.size.clamp(1, u16::MAX as usize) as u16,
        ..MoonlyLight::default()
    }
}

fn adapt_spot(light: &SpotLight, shadow_map: &PointLightShadowMap) -> MoonlyLight {
    MoonlyLight {
        light_type: MoonlyLightType::Spot,
        flags: native_flags(light.shadow_maps_enabled),
        color: linear_color(light.color, light.intensity / FULL_SPHERE_STERADIANS),
        range: light.range,
        cone_angle: light.outer_angle * 2.0,
        inner_cone_angle: light.inner_angle * 2.0,
        shadow_near_z: light.shadow_map_near_z,
        smap_size_override: shadow_map.size.clamp(1, u16::MAX as usize) as u16,
        ..MoonlyLight::default()
    }
}

fn adapt_directional(light: &DirectionalLight) -> MoonlyLight {
    MoonlyLight {
        light_type: MoonlyLightType::Direct,
        flags: native_flags(light.shadow_maps_enabled),
        color: linear_color(light.color, light.illuminance),
        ..MoonlyLight::default()
    }
}

const fn native_flags(shadows: bool) -> MoonlyLightFlags {
    let mut flags = MoonlyLightFlags::NATIVE;
    if shadows {
        flags = flags.union(MoonlyLightFlags::SHADOW);
    }
    flags
}

fn linear_color(color: Color, intensity: f32) -> Vec3 {
    let linear = color.to_linear();
    Vec3::new(linear.red, linear.green, linear.blue) * intensity
}

#[cfg(test)]
mod tests;

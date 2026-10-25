use bevy::prelude::*;

use crate::{MoonlyLight, MoonlyLightFlags, MoonlyLightType};

const FULL_SPHERE_STERADIANS: f32 = 4.0 * std::f32::consts::PI;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct MoonlyNativeLight;

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
    mut lights: Query<(
        Entity,
        &PointLight,
        Option<&mut MoonlyLight>,
        Has<MoonlyNativeLight>,
    )>,
) {
    for (entity, native, current, managed) in &mut lights {
        if current.is_some() && !managed {
            continue;
        }
        let adapted = adapt_point(native);
        update_or_insert(&mut commands, entity, current, adapted);
    }
}

pub(crate) fn sync_native_spot_lights(
    mut commands: Commands,
    mut lights: Query<(
        Entity,
        &SpotLight,
        Option<&mut MoonlyLight>,
        Has<MoonlyNativeLight>,
    )>,
) {
    for (entity, native, current, managed) in &mut lights {
        if current.is_some() && !managed {
            continue;
        }
        let adapted = adapt_spot(native);
        update_or_insert(&mut commands, entity, current, adapted);
    }
}

pub(crate) fn sync_native_directional_lights(
    mut commands: Commands,
    mut lights: Query<(
        Entity,
        &DirectionalLight,
        Option<&mut MoonlyLight>,
        Has<MoonlyNativeLight>,
    )>,
) {
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

fn adapt_point(light: &PointLight) -> MoonlyLight {
    MoonlyLight {
        light_type: MoonlyLightType::Point,
        flags: native_flags(light.shadow_maps_enabled, false),
        color: linear_color(light.color, light.intensity / FULL_SPHERE_STERADIANS),
        range: light.range,
        ..MoonlyLight::default()
    }
}

fn adapt_spot(light: &SpotLight) -> MoonlyLight {
    MoonlyLight {
        light_type: MoonlyLightType::Spot,
        flags: native_flags(light.shadow_maps_enabled, true),
        color: linear_color(light.color, light.intensity / FULL_SPHERE_STERADIANS),
        range: light.range,
        cone_angle: light.outer_angle,
        inner_cone_angle: light.inner_angle,
        ..MoonlyLight::default()
    }
}

fn adapt_directional(light: &DirectionalLight) -> MoonlyLight {
    MoonlyLight {
        light_type: MoonlyLightType::Direct,
        flags: native_flags(light.shadow_maps_enabled, false),
        color: linear_color(light.color, light.illuminance),
        ..MoonlyLight::default()
    }
}

const fn native_flags(shadows: bool, volumetric: bool) -> MoonlyLightFlags {
    let mut flags = MoonlyLightFlags::GI_ENABLED;
    if shadows {
        flags = flags.union(MoonlyLightFlags::SHADOW);
    }
    if volumetric {
        flags = flags.union(MoonlyLightFlags::VOLUMETRIC);
    }
    flags
}

fn linear_color(color: Color, intensity: f32) -> Vec3 {
    let linear = color.to_linear();
    Vec3::new(linear.red, linear.green, linear.blue) * intensity
}

#[cfg(test)]
mod tests;

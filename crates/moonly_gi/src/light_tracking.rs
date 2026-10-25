use bevy::{ecs::system::SystemParam, prelude::*};
use moonly_core::{HemisphereTracker, MoonlyLight, MoonlyLightType, WeatherEnvironment};

use crate::{GiRayGeometry, GiRayQuery, StaticGiGeometry};

const SAMPLES_PER_UPDATE: usize = 5;
const STATIC_RAY_DISTANCE_METERS: f32 = 50.0;
const SUN_RAY_DISTANCE_METERS: f32 = 500.0;
const RAY_ORIGIN_BIAS_METERS: f32 = 0.02;
const LOCAL_LIGHT_SCALE: f32 = 0.2;
const LOCAL_LIGHT_FLOW: f32 = 0.1;

#[derive(SystemParam)]
pub(crate) struct LightTrackingInputs<'w, 's> {
    environment: Option<Res<'w, WeatherEnvironment>>,
    geometry: Res<'w, StaticGiGeometry>,
    lights: Query<'w, 's, &'static MoonlyLight>,
    trackers: Query<'w, 's, (&'static GlobalTransform, &'static mut HemisphereTracker)>,
}

/// Trace incremental visibility rays and approximate nearby local lights.
pub(crate) fn update_light_tracking(inputs: LightTrackingInputs) {
    let LightTrackingInputs {
        environment,
        geometry,
        lights,
        mut trackers,
    } = inputs;
    let Some(environment) = environment else {
        return;
    };
    for (transform, mut tracker) in &mut trackers {
        let origin = transform.translation();
        for _ in 0..SAMPLES_PER_UPDATE {
            let Some((index, direction)) = tracker.next_sample() else {
                continue;
            };
            let visible =
                trace_visibility(&geometry, origin, direction, STATIC_RAY_DISTANCE_METERS);
            tracker.record_sample(index, visible);
        }
        let toward_sun = -environment.sun_direction.normalize_or_zero();
        tracker.sun_value =
            trace_visibility(&geometry, origin, toward_sun, SUN_RAY_DISTANCE_METERS);
        tracker.approximate_local = approximate_local_lights(origin, &lights, &geometry);
    }
}

fn trace_visibility(
    geometry: &StaticGiGeometry,
    origin: Vec3,
    direction: Vec3,
    distance: f32,
) -> f32 {
    if direction == Vec3::ZERO {
        return 0.0;
    }
    let biased_origin = origin + direction * RAY_ORIGIN_BIAS_METERS;
    let ray = GiRayQuery::new(biased_origin, direction, RAY_ORIGIN_BIAS_METERS, distance);
    if geometry.nearest_hit(ray).is_some() {
        0.0
    } else {
        1.0
    }
}

fn approximate_local_lights(
    origin: Vec3,
    lights: &Query<&MoonlyLight>,
    geometry: &StaticGiGeometry,
) -> Vec3 {
    lights
        .iter()
        .filter(|light| !matches!(light.light_type, MoonlyLightType::Direct))
        .map(|light| local_light_contribution(origin, light, geometry))
        .sum()
}

fn local_light_contribution(
    origin: Vec3,
    light: &MoonlyLight,
    geometry: &StaticGiGeometry,
) -> Vec3 {
    let delta = light.position - origin;
    let distance = delta.length();
    if distance <= RAY_ORIGIN_BIAS_METERS || distance >= light.range || light.range <= 0.0 {
        return Vec3::ZERO;
    }
    let direction = delta / distance;
    if matches!(light.light_type, MoonlyLightType::Spot)
        && direction.dot(-light.direction) < (light.cone_angle * 0.5).cos()
    {
        return Vec3::ZERO;
    }
    if trace_visibility(geometry, origin, direction, distance) == 0.0 {
        return Vec3::ZERO;
    }
    let attenuation = (1.0 - distance / light.range).powi(2);
    light.color * attenuation * (LOCAL_LIGHT_SCALE + LOCAL_LIGHT_FLOW)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GiTriangle;

    fn app_with_roof(with_roof: bool) -> App {
        let mut app = App::new();
        app.init_resource::<WeatherEnvironment>();
        app.init_resource::<StaticGiGeometry>();
        app.add_systems(Update, update_light_tracking);
        let mut triangles = Vec::new();
        if with_roof {
            let roof = GiTriangle::new(
                [
                    Vec3::new(-4.0, 2.0, -4.0),
                    Vec3::new(4.0, 2.0, 4.0),
                    Vec3::new(-4.0, 2.0, 4.0),
                ],
                false,
            );
            let roof_pair = GiTriangle::new(
                [
                    Vec3::new(-4.0, 2.0, -4.0),
                    Vec3::new(4.0, 2.0, -4.0),
                    Vec3::new(4.0, 2.0, 4.0),
                ],
                false,
            );
            triangles.extend(roof.into_iter().chain(roof_pair));
        }
        app.world_mut()
            .resource_mut::<StaticGiGeometry>()
            .rebuild(&mut triangles);
        app.world_mut()
            .spawn((GlobalTransform::default(), HemisphereTracker::default()));
        app
    }

    #[test]
    fn roof_blocks_sky_and_sun_but_not_lower_cube_faces() {
        let mut open = app_with_roof(false);
        let mut roofed = app_with_roof(true);
        let geometry = roofed.world().resource::<StaticGiGeometry>();
        for direction in moonly_core::HEMI_SAMPLE_DIRS
            .iter()
            .filter(|direction| direction.y > 0.0)
        {
            assert!(
                trace_visibility(geometry, Vec3::ZERO, *direction, 50.0) < 0.01,
                "missed {direction:?}"
            );
        }
        for _ in 0..300 {
            open.update();
            roofed.update();
        }
        let mut open_query = open.world_mut().query::<&HemisphereTracker>();
        let open = open_query.single(open.world()).cloned();
        let mut roofed_query = roofed.world_mut().query::<&HemisphereTracker>();
        let roofed = roofed_query.single(roofed.world()).cloned();
        assert!(open.is_ok() && roofed.is_ok());
        let (Ok(open), Ok(roofed)) = (open, roofed) else {
            return;
        };
        assert!(open.hemi_value > 0.99);
        assert!(
            roofed.hemi_value < 0.01,
            "hemi={} energy={:?}",
            roofed.hemi_value,
            roofed.sample_energy
        );
        assert!(roofed.hemi_cube.get(1).is_some_and(|value| *value < 0.01));
        assert!(roofed.hemi_cube.get(4).is_some_and(|value| *value > 0.99));
        assert!(open.sun_value > 0.99);
        assert!(roofed.sun_value < 0.01);
    }
}

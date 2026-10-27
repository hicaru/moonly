use bevy::{
    pbr::{DistanceFog, FogFalloff},
    prelude::*,
};
use moonly_core::WeatherEnvironment;

use crate::camera::MoonlyCamera;

pub(crate) fn sync_fog_cameras(
    environment: Option<Res<WeatherEnvironment>>,
    mut cameras: Query<&mut DistanceFog, With<MoonlyCamera>>,
) {
    let Some(environment) = environment else {
        return;
    };
    for mut fog in &mut cameras {
        fog.color = Color::linear_rgb(
            environment.fog_color.x,
            environment.fog_color.y,
            environment.fog_color.z,
        );
        fog.falloff = FogFalloff::Exponential {
            density: environment.fog_density,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_fog_tracks_weather_density_and_color() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, crate::MoonlyRenderPlugin));
        app.world_mut().insert_resource(WeatherEnvironment {
            fog_density: 0.025,
            ..WeatherEnvironment::default()
        });
        let entity = app
            .world_mut()
            .spawn((MoonlyCamera::default(), DistanceFog::default()))
            .id();
        app.update();
        let fog = app.world().get::<DistanceFog>(entity).cloned();
        assert!(fog.is_some_and(|fog| {
            fog.color.to_linear().red > 0.3
                && matches!(fog.falloff, FogFalloff::Exponential { density } if (density - 0.025).abs() < 1.0e-6)
        }));
    }

    #[test]
    fn native_forward_preserves_explicit_fog() {
        let mut app = App::new();
        app.insert_resource(moonly_core::MoonlyNativeForward(true));
        app.add_plugins((MinimalPlugins, crate::MoonlyRenderPlugin));
        let color = Color::srgb(0.1, 0.2, 0.3);
        let entity = app
            .world_mut()
            .spawn((
                MoonlyCamera::default(),
                DistanceFog {
                    color,
                    falloff: FogFalloff::Linear {
                        start: 1.0,
                        end: 2.0,
                    },
                    ..DistanceFog::default()
                },
            ))
            .id();
        app.update();
        let fog = app.world().get::<DistanceFog>(entity).cloned();
        assert!(fog.is_some_and(|fog| {
            fog.color == color
                && matches!(fog.falloff, FogFalloff::Linear { start, end }
                    if (start - 1.0).abs() < f32::EPSILON && (end - 2.0).abs() < f32::EPSILON)
        }));
    }
}

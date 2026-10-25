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
}

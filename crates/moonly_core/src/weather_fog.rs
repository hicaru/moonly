use bevy::prelude::*;

use crate::WeatherEnvironment;

impl WeatherEnvironment {
    /// Evaluate exponential distance fog with height attenuation.
    #[must_use]
    pub fn density_fog(&self, world_pos: Vec3, view_pos: Vec3) -> f32 {
        let distance = world_pos.distance(view_pos);
        let height = (-self.fog_height_falloff * (world_pos.y - self.fog_height_reference)).exp();
        1.0 - (-self.fog_density.max(0.0) * distance * height).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_density_keeps_scene_clear() {
        let environment = WeatherEnvironment::default();
        assert!(
            environment
                .density_fog(Vec3::new(0.0, 1.0, -50.0), Vec3::ZERO)
                .abs()
                < 1.0e-6
        );
    }

    #[test]
    fn density_fog_increases_with_distance() {
        let environment = WeatherEnvironment {
            fog_density: 0.02,
            ..WeatherEnvironment::default()
        };
        let near = environment.density_fog(Vec3::new(0.0, 0.0, -2.0), Vec3::ZERO);
        let far = environment.density_fog(Vec3::new(0.0, 0.0, -20.0), Vec3::ZERO);
        assert!(far > near);
        assert!(near > 0.0);
    }

    #[test]
    fn higher_geometry_receives_less_height_fog() {
        let environment = WeatherEnvironment {
            fog_density: 0.02,
            fog_height_falloff: 0.1,
            ..WeatherEnvironment::default()
        };
        let low = environment.density_fog(Vec3::new(0.0, 0.0, -20.0), Vec3::ZERO);
        let high = environment.density_fog(Vec3::new(0.0, 10.0, -20.0), Vec3::ZERO);
        assert!(high < low);
    }
}

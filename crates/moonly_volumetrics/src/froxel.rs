use bevy::{
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
};

/// Grid dimensions for the Moonly froxel density texture.
pub const FROXEL_GRID_SIZE: UVec3 = UVec3::new(16, 8, 16);

/// Configuration for the Moonly froxel fog pass.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct MoonlyFroxelConfig {
    /// Enable the froxel density pass.
    pub enabled: bool,
    /// World-space minimum grid corner.
    pub grid_origin: Vec3,
    /// World-space grid extent.
    pub grid_extent: Vec3,
    /// Density multiplier applied to texture cells.
    pub density: f32,
    /// Maximum raymarch steps per pixel.
    pub steps: u32,
}

impl Default for MoonlyFroxelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            grid_origin: Vec3::new(-16.0, -4.0, -16.0),
            grid_extent: Vec3::new(32.0, 12.0, 32.0),
            density: 0.01,
            steps: 24,
        }
    }
}

impl ExtractResource<RenderApp> for MoonlyFroxelConfig {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

impl MoonlyFroxelConfig {
    /// Convert a world position to integer froxel coordinates.
    #[must_use]
    pub fn cell_from_world(&self, position: Vec3) -> Option<UVec3> {
        let normalized = (position - self.grid_origin) / self.grid_extent;
        let dimensions = FROXEL_GRID_SIZE.as_vec3();
        if normalized.cmplt(Vec3::ZERO).any() || normalized.cmpge(Vec3::ONE).any() {
            return None;
        }
        Some((normalized * dimensions).as_uvec3())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_cells_cover_the_grid_without_outside_samples() {
        let config = MoonlyFroxelConfig::default();
        let center = config.grid_origin + config.grid_extent * 0.5;
        assert_eq!(config.cell_from_world(center), Some(UVec3::new(8, 4, 8)));
        assert_eq!(config.cell_from_world(config.grid_origin - Vec3::ONE), None);
        assert_eq!(
            config.cell_from_world(config.grid_origin + config.grid_extent),
            None
        );
    }
}

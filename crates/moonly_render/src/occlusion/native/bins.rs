use bevy::{
    prelude::*,
    render::{render_resource::ShaderType, view::ExtractedView},
};
use moonly_core::{MoonlyLight, MoonlyLightType};

use crate::{accumulate::MoonlySpotUniform, light_volume::clip_from_local};

pub(crate) const TILE_SIZE: u32 = 64;

#[derive(Default)]
pub(crate) struct NativeBuildScratch {
    pub(crate) lights: Vec<MoonlySpotUniform>,
    pub(crate) bounds: Vec<ProjectionBounds>,
    pub(crate) tiles: Vec<NativeTile>,
    pub(crate) indices: Vec<u32>,
}

#[derive(ShaderType, Default, Clone, Copy)]
pub(crate) struct NativeTile {
    pub(crate) offset: u32,
    pub(crate) count: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct ProjectionBounds {
    min: Vec2,
    max: Vec2,
    full: bool,
}

impl ProjectionBounds {
    const fn empty() -> Self {
        Self {
            min: Vec2::splat(f32::INFINITY),
            max: Vec2::splat(f32::NEG_INFINITY),
            full: false,
        }
    }

    fn append(&mut self, matrix: Mat4, point: Vec3) {
        let clip = matrix * point.extend(1.0);
        if clip.w <= 0.0 {
            self.full = true;
            return;
        }
        let ndc = clip.xy() / clip.w;
        self.min = self.min.min(ndc);
        self.max = self.max.max(ndc);
    }
}

pub(crate) fn project_bounds(light: &MoonlyLight, view: &ExtractedView) -> ProjectionBounds {
    let matrix = clip_from_local(light, view);
    let mut bounds = ProjectionBounds::empty();
    if light.light_type == MoonlyLightType::Point {
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    bounds.append(matrix, Vec3::new(x, y, z));
                }
            }
        }
    } else {
        append_cone(&mut bounds, matrix);
    }
    bounds
}

fn append_cone(bounds: &mut ProjectionBounds, matrix: Mat4) {
    let cap: [Vec3; 16] = std::array::from_fn(|index| {
        let angle = std::f32::consts::TAU * index as f32 / 16.0;
        Vec3::new(0.5 * angle.cos(), 0.5 * angle.sin(), 1.0)
    });
    for (current, next) in cap.iter().zip(cap.iter().cycle().skip(1)) {
        append_triangle(bounds, matrix, [Vec3::ZERO, *current, *next]);
        append_triangle(bounds, matrix, [Vec3::Z, *current, *next]);
    }
}

fn append_triangle(bounds: &mut ProjectionBounds, matrix: Mat4, points: [Vec3; 3]) {
    let clip = points.map(|point| matrix * point.extend(1.0));
    let mut polygon = [Vec4::ZERO; 4];
    let mut count = 0;
    for edge in 0..3 {
        let Some(current) = clip.get(edge).copied() else {
            continue;
        };
        let Some(next) = clip.get((edge + 1) % 3).copied() else {
            continue;
        };
        if current.w > 0.0
            && let Some(slot) = polygon.get_mut(count)
        {
            *slot = current;
            count += 1;
        }
        if (current.w > 0.0) != (next.w > 0.0) {
            let fraction = current.w / (current.w - next.w);
            if let Some(slot) = polygon.get_mut(count) {
                *slot = current + (next - current) * fraction;
                count += 1;
            }
        }
    }
    for point in polygon.into_iter().take(count) {
        if point.w > 0.0 {
            let ndc = point.xy() / point.w;
            bounds.min = bounds.min.min(ndc);
            bounds.max = bounds.max.max(ndc);
        }
    }
}

pub(crate) fn build(view: &ExtractedView, scratch: &mut NativeBuildScratch) -> UVec4 {
    let grid = tile_grid(view);
    let count = grid.x.saturating_mul(grid.y) as usize;
    scratch.tiles.clear();
    scratch.tiles.resize(count, NativeTile::default());
    scratch.indices.clear();
    for (index, bounds) in scratch.bounds.iter().copied().enumerate() {
        let (min, max) = tile_rect(bounds, grid);
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let index_in_grid = (y * grid.x + x) as usize;
                let Some(tile) = scratch.tiles.get_mut(index_in_grid) else {
                    continue;
                };
                tile.count = tile.count.saturating_add(1);
                scratch.indices.push(index as u32);
            }
        }
    }
    let mut offset = 0_u32;
    for tile in &mut scratch.tiles {
        tile.offset = offset;
        offset = offset.saturating_add(tile.count);
    }
    UVec4::new(grid.x, grid.y, TILE_SIZE, scratch.lights.len() as u32)
}

fn tile_grid(view: &ExtractedView) -> UVec2 {
    UVec2::new(
        view.viewport.z.div_ceil(TILE_SIZE).max(1),
        view.viewport.w.div_ceil(TILE_SIZE).max(1),
    )
}

fn tile_rect(bounds: ProjectionBounds, grid: UVec2) -> (UVec2, UVec2) {
    if bounds.full {
        return (UVec2::ZERO, grid.saturating_sub(UVec2::ONE));
    }
    let scale = vec2(0.5, -0.5);
    let offset = Vec2::splat(0.5);
    let min = (vec2(bounds.min.x, bounds.max.y) * scale + offset).clamp(Vec2::ZERO, Vec2::ONE);
    let max = (vec2(bounds.max.x, bounds.min.y) * scale + offset).clamp(Vec2::ZERO, Vec2::ONE);
    let last = grid.saturating_sub(UVec2::ONE);
    let min = (min * grid.as_vec2()).as_uvec2().min(last);
    let max = (max * grid.as_vec2()).as_uvec2().min(last);
    (min, max)
}

#[cfg(test)]
mod tests {
    use bevy::render::{render_resource::TextureFormat, view::RetainedViewEntity};

    use super::*;

    #[test]
    fn ndc_bounds_cover_the_whole_tile_grid() {
        let bounds = ProjectionBounds {
            min: Vec2::splat(-1.0),
            max: Vec2::splat(1.0),
            full: false,
        };
        assert_eq!(
            tile_rect(bounds, UVec2::new(20, 12)),
            (UVec2::ZERO, UVec2::new(19, 11))
        );
    }

    #[test]
    fn native_spot_volume_covers_the_central_tile() {
        let position = Vec3::new(-2.0, 5.0, 3.0);
        let direction = (Vec3::new(0.0, 0.8, 0.0) - position).normalize();
        let light = MoonlyLight {
            light_type: MoonlyLightType::Spot,
            position,
            direction,
            right: Vec3::Y.cross(direction).normalize(),
            range: 18.0,
            cone_angle: 2.8,
            ..MoonlyLight::default()
        };
        let view = ExtractedView {
            retained_view_entity: RetainedViewEntity::new(Entity::PLACEHOLDER.into(), None, 0),
            clip_from_view: moonly_core::perspective_rh_mat4(
                std::f32::consts::FRAC_PI_4,
                1280.0 / 720.0,
                0.1,
                1000.0,
            ),
            world_from_view: GlobalTransform::from(
                Transform::from_xyz(5.5, 4.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
            ),
            clip_from_world: None,
            target_format: TextureFormat::Rgba16Float,
            viewport: UVec4::new(0, 0, 1280, 720),
            color_grading: bevy::render::view::ColorGrading::default(),
            invert_culling: false,
        };
        let bounds = project_bounds(&light, &view);
        let (min, max) = tile_rect(bounds, UVec2::new(20, 12));
        assert!(
            min.x <= 10 && max.x >= 10,
            "x: {min}..{max}, bounds: {:?}..{:?}",
            bounds.min,
            bounds.max
        );
        assert!(
            min.y <= 5 && max.y >= 5,
            "y: {min}..{max}, bounds: {:?}..{:?}",
            bounds.min,
            bounds.max
        );
    }
}

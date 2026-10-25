use bevy::{camera::primitives::Aabb, prelude::*};
use moonly_core::MoonlyConfig;
use moonly_render::MoonlyCamera;

use crate::{
    hom_geometry::MoonlyHomGeometry,
    occ_rasterizer::{OCC_DIM_0, OccRasterizer, OccTestRect},
};

const CLIP_EPSILON: f32 = 1.0e-6;

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct MoonlyHomView {
    rasterizer: OccRasterizer,
    clip_from_world: Mat4,
    camera_position: Vec3,
    pub(crate) enabled: bool,
}

impl Default for MoonlyHomView {
    fn default() -> Self {
        Self {
            rasterizer: OccRasterizer::default(),
            clip_from_world: Mat4::IDENTITY,
            camera_position: Vec3::ZERO,
            enabled: false,
        }
    }
}

impl MoonlyHomView {
    #[must_use]
    pub fn aabb_visible(&self, aabb: Aabb) -> bool {
        let camera_position = Vec3A::from(self.camera_position);
        let camera_inside =
            aabb.min().cmple(camera_position).all() && camera_position.cmple(aabb.max()).all();
        if !self.enabled || camera_inside {
            return true;
        }
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        let mut depth = 1.0_f32;
        for x in [aabb.min().x, aabb.max().x] {
            for y in [aabb.min().y, aabb.max().y] {
                for z in [aabb.min().z, aabb.max().z] {
                    let clip = self.clip_from_world * Vec3::new(x, y, z).extend(1.0);
                    if clip.w <= CLIP_EPSILON || clip.z <= CLIP_EPSILON {
                        return true;
                    }
                    let ndc = clip.xyz() / clip.w;
                    min = min.min(ndc.truncate());
                    max = max.max(ndc.truncate());
                    depth = depth.min(ndc.z);
                }
            }
        }
        let rect = OccTestRect {
            x0: screen_coordinate(min.x),
            y0: screen_coordinate(min.y),
            x1: screen_coordinate(max.x),
            y1: screen_coordinate(max.y),
            depth,
        };
        self.rasterizer.test_monolith(rect) > 0
    }
}

type HomCameraQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Camera,
        &'static GlobalTransform,
        Option<&'static mut MoonlyHomView>,
    ),
    With<MoonlyCamera>,
>;

pub(crate) fn update_hom_views(
    mut commands: Commands,
    config: Res<MoonlyConfig>,
    geometry: Res<MoonlyHomGeometry>,
    mut cameras: HomCameraQuery,
) {
    for (entity, camera, transform, existing) in &mut cameras {
        let enabled =
            config.enable_occ_rasterizer && camera.is_active && !geometry.triangles.is_empty();
        if let Some(mut view) = existing {
            update_view(&mut view, camera, transform, enabled, &geometry);
            continue;
        }
        let mut view = MoonlyHomView::default();
        update_view(&mut view, camera, transform, enabled, &geometry);
        commands.entity(entity).insert(view);
    }
}

fn update_view(
    view: &mut MoonlyHomView,
    camera: &Camera,
    transform: &GlobalTransform,
    enabled: bool,
    geometry: &MoonlyHomGeometry,
) {
    view.clip_from_world = camera.clip_from_view() * transform.to_matrix().inverse();
    view.camera_position = transform.translation();
    view.enabled = enabled;
    view.rasterizer.clear();
    if enabled {
        for triangle in &geometry.triangles {
            rasterize_triangle(view, triangle);
        }
        view.rasterizer.propagate();
    }
}

fn rasterize_triangle(view: &mut MoonlyHomView, triangle: &crate::hom_geometry::HomTriangle) {
    let [v0, v1, v2] = triangle.vertices;
    let normal = (v1 - v0).cross(v2 - v0);
    let facing = normal.dot(view.camera_position - v0) > 0.0;
    if !triangle.double_sided && !facing {
        return;
    }
    let clip = triangle
        .vertices
        .map(|vertex| view.clip_from_world * vertex.extend(1.0));
    let (polygon, count) = clip_near(clip);
    for index in 1..count.saturating_sub(1) {
        let (Some(v0), Some(v1), Some(v2)) =
            (polygon.first(), polygon.get(index), polygon.get(index + 1))
        else {
            continue;
        };
        let (Some(v0), Some(v1), Some(v2)) =
            (screen_vertex(*v0), screen_vertex(*v1), screen_vertex(*v2))
        else {
            continue;
        };
        view.rasterizer.rasterize_triangle(v0, v1, v2);
    }
}

fn clip_near(vertices: [Vec4; 3]) -> ([Vec4; 4], usize) {
    let mut output = [Vec4::ZERO; 4];
    let mut count = 0;
    for index in 0..vertices.len() {
        let Some(current) = vertices.get(index).copied() else {
            continue;
        };
        let Some(previous) = vertices.get((index + 2) % 3).copied() else {
            continue;
        };
        let current_inside = current.z > CLIP_EPSILON && current.w > CLIP_EPSILON;
        let previous_inside = previous.z > CLIP_EPSILON && previous.w > CLIP_EPSILON;
        if current_inside != previous_inside {
            let amount = (CLIP_EPSILON - previous.z) / (current.z - previous.z);
            push_vertex(&mut output, &mut count, previous.lerp(current, amount));
        }
        if current_inside {
            push_vertex(&mut output, &mut count, current);
        }
    }
    (output, count)
}

fn push_vertex(output: &mut [Vec4; 4], count: &mut usize, vertex: Vec4) {
    if let Some(slot) = output.get_mut(*count) {
        *slot = vertex;
        *count += 1;
    }
}

fn screen_vertex(clip: Vec4) -> Option<Vec3> {
    if clip.w <= CLIP_EPSILON {
        return None;
    }
    let ndc = clip.xyz() / clip.w;
    Some(Vec3::new(
        (ndc.x * 0.5 + 0.5) * OCC_DIM_0 as f32,
        (0.5 - ndc.y * 0.5) * OCC_DIM_0 as f32,
        ndc.z,
    ))
}

fn screen_coordinate(value: f32) -> usize {
    ((value * 0.5 + 0.5) * OCC_DIM_0 as f32)
        .floor()
        .clamp(0.0, (OCC_DIM_0 - 1) as f32) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hom_geometry::HomTriangle;

    #[test]
    fn rasterized_wall_hides_only_geometry_behind_it() {
        let mut view = MoonlyHomView {
            clip_from_world: moonly_core::perspective_rh_mat4(
                std::f32::consts::FRAC_PI_2,
                1.0,
                0.1,
                100.0,
            ),
            enabled: true,
            ..MoonlyHomView::default()
        };
        for vertices in [
            [
                Vec3::new(-2.0, -2.0, -5.0),
                Vec3::new(2.0, -2.0, -5.0),
                Vec3::new(-2.0, 2.0, -5.0),
            ],
            [
                Vec3::new(2.0, -2.0, -5.0),
                Vec3::new(2.0, 2.0, -5.0),
                Vec3::new(-2.0, 2.0, -5.0),
            ],
        ] {
            rasterize_triangle(
                &mut view,
                &HomTriangle {
                    vertices,
                    double_sided: false,
                },
            );
        }
        view.rasterizer.propagate();
        assert!(!view.aabb_visible(test_aabb(Vec3::new(0.0, 0.0, -10.0))));
        assert!(view.aabb_visible(test_aabb(Vec3::new(0.0, 0.0, -1.0))));
        assert!(view.aabb_visible(test_aabb(Vec3::new(20.0, 0.0, -10.0))));
    }

    fn test_aabb(center: Vec3) -> Aabb {
        Aabb::from_min_max(center - Vec3::splat(0.25), center + Vec3::splat(0.25))
    }
}

use bevy::{
    prelude::*,
    render::{
        render_resource::{Buffer, BufferDescriptor, BufferUsages},
        renderer::{RenderDevice, RenderQueue},
        view::ExtractedView,
    },
};
use moonly_core::{MoonlyLight, MoonlyLightType};

use crate::point_volume::{MoonlyPointVolume, init_point_volume};

pub(crate) const CONE_INDEX_COUNT: u32 = 96;

#[derive(Resource)]
pub(crate) struct MoonlyLightVolume {
    pub vertices: Buffer,
    pub indices: Buffer,
    pub point: MoonlyPointVolume,
}

pub(crate) fn init_light_volume(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let mut positions = Vec::with_capacity(18);
    positions.push(Vec3::ZERO);
    for index in 0..16 {
        let angle = std::f32::consts::TAU * index as f32 / 16.0;
        positions.push(Vec3::new(0.5 * angle.cos(), 0.5 * angle.sin(), 1.0));
    }
    positions.push(Vec3::Z);
    let mut indices = Vec::with_capacity(CONE_INDEX_COUNT as usize);
    for index in 0..16_u16 {
        let current = index + 1;
        let next = (index + 1) % 16 + 1;
        indices.extend_from_slice(&[0, next, current, 17, current, next]);
    }
    let position_bytes: Vec<u8> = positions
        .iter()
        .flat_map(|value| [value.x, value.y, value.z])
        .flat_map(f32::to_le_bytes)
        .collect();
    let index_bytes: Vec<u8> = indices
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let vertices = create_buffer(
        &device,
        &queue,
        "moonly_spot_cone_vertices",
        &position_bytes,
        BufferUsages::VERTEX,
    );
    let indices = create_buffer(
        &device,
        &queue,
        "moonly_spot_cone_indices",
        &index_bytes,
        BufferUsages::INDEX,
    );
    commands.insert_resource(MoonlyLightVolume {
        vertices,
        indices,
        point: init_point_volume(&device, &queue),
    });
}

fn create_buffer(
    device: &RenderDevice,
    queue: &RenderQueue,
    label: &str,
    bytes: &[u8],
    usage: BufferUsages,
) -> Buffer {
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: bytes.len() as u64,
        usage: usage | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytes);
    buffer
}

pub(crate) fn clip_from_local(light: &MoonlyLight, view: &ExtractedView) -> Mat4 {
    let clip_from_world = view
        .clip_from_world
        .unwrap_or_else(|| view.clip_from_view * view.world_from_view.to_matrix().inverse());
    if light.light_type == MoonlyLightType::Point {
        let local_from_light = Mat4::from_scale_rotation_translation(
            Vec3::splat(light.range),
            Quat::IDENTITY,
            light.position,
        );
        return clip_from_world * local_from_light;
    }
    clip_from_local_with_camera(light, clip_from_world)
}

fn clip_from_local_with_camera(light: &MoonlyLight, clip_from_world: Mat4) -> Mat4 {
    let direction = light.direction.normalize_or_zero();
    let right = light.right.normalize_or_zero();
    let up = direction.cross(right).normalize_or_zero();
    let radius = 2.0 * light.range * (light.cone_angle * 0.5).tan();
    let local_from_light = Mat4::from_cols(
        (right * radius).extend(0.0),
        (up * radius).extend(0.0),
        (direction * light.range).extend(0.0),
        light.position.extend(1.0),
    );
    clip_from_world * local_from_light
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cone_transform_reaches_range_and_cap_radius() {
        let light = MoonlyLight::spot(Vec3::ZERO, Vec3::Z, Vec3::ONE, 10.0, 1.0);
        let matrix = clip_from_local_with_camera(&light, Mat4::IDENTITY);
        assert!(matrix.transform_point3(Vec3::Z).distance(Vec3::Z * 10.0) < 1.0e-5);
        let radius = 10.0 * 0.5_f32.tan();
        assert!(
            matrix
                .transform_point3(Vec3::new(0.5, 0.0, 1.0))
                .distance(Vec3::new(radius, 0.0, 10.0))
                < 1.0e-5
        );
    }
}

use bevy::{
    prelude::*,
    render::{
        render_resource::{Buffer, BufferDescriptor, BufferUsages},
        renderer::{RenderDevice, RenderQueue},
    },
};

const LATITUDE_COUNT: u16 = 8;
const LONGITUDE_COUNT: u16 = 16;
pub(crate) const POINT_INDEX_COUNT: u32 = LATITUDE_COUNT as u32 * LONGITUDE_COUNT as u32 * 6;

pub(crate) struct MoonlyPointVolume {
    pub vertices: Buffer,
    pub indices: Buffer,
}

pub(crate) fn init_point_volume(device: &RenderDevice, queue: &RenderQueue) -> MoonlyPointVolume {
    let (positions, indices) = sphere_geometry();
    let position_bytes: Vec<u8> = positions
        .iter()
        .flat_map(|value| [value.x, value.y, value.z])
        .flat_map(f32::to_le_bytes)
        .collect();
    let index_bytes: Vec<u8> = indices
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    MoonlyPointVolume {
        vertices: create_buffer(device, queue, "moonly_point_vertices", &position_bytes),
        indices: create_buffer(device, queue, "moonly_point_indices", &index_bytes),
    }
}

fn sphere_geometry() -> (Vec<Vec3>, Vec<u16>) {
    let mut positions = Vec::new();
    for latitude in 0..=LATITUDE_COUNT {
        let theta = std::f32::consts::PI * f32::from(latitude) / f32::from(LATITUDE_COUNT);
        for longitude in 0..=LONGITUDE_COUNT {
            let phi = std::f32::consts::TAU * f32::from(longitude) / f32::from(LONGITUDE_COUNT);
            positions.push(Vec3::new(
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            ));
        }
    }
    let mut indices = Vec::with_capacity(POINT_INDEX_COUNT as usize);
    for latitude in 0..LATITUDE_COUNT {
        for longitude in 0..LONGITUDE_COUNT {
            let a = latitude * (LONGITUDE_COUNT + 1) + longitude;
            let b = a + LONGITUDE_COUNT + 1;
            indices.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    (positions, indices)
}

fn create_buffer(device: &RenderDevice, queue: &RenderQueue, label: &str, bytes: &[u8]) -> Buffer {
    let buffer = device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: bytes.len() as u64,
        usage: BufferUsages::VERTEX | BufferUsages::INDEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytes);
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_has_closed_unit_surface_and_expected_index_count() {
        let (positions, indices) = sphere_geometry();
        assert_eq!(indices.len(), POINT_INDEX_COUNT as usize);
        assert!(
            positions
                .iter()
                .all(|position| { position.length() > 0.999 && position.length() < 1.001 })
        );
        assert!(
            indices
                .iter()
                .all(|index| usize::from(*index) < positions.len())
        );
    }
}

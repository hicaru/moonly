use std::sync::{
    Mutex, RwLock,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{Receiver, Sender, channel},
};

use bevy::{
    prelude::*,
    render::{
        render_resource::{Buffer, BufferDescriptor, BufferUsages, CommandEncoder, MapMode},
        renderer::RenderDevice,
    },
};
use wgpu::{QuerySet, QuerySetDescriptor, QueryType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MoonlyLightKey {
    pub source_entity: Option<Entity>,
    pub omnipart_face: Option<u8>,
}

#[derive(Component)]
pub(crate) struct MoonlyOcclusionState {
    pub query_set: QuerySet,
    readback: [Buffer; 2],
    current_keys: Mutex<Vec<MoonlyLightKey>>,
    scheduled_keys: [Mutex<Vec<MoonlyLightKey>>; 2],
    pending: AtomicBool,
    slot: AtomicUsize,
    query_count: usize,
    visible: RwLock<Vec<(MoonlyLightKey, bool)>>,
    sender: Sender<(usize, Vec<u64>)>,
    receiver: Mutex<Receiver<(usize, Vec<u64>)>>,
}

impl MoonlyOcclusionState {
    pub(crate) fn new(device: &RenderDevice, count: usize) -> Self {
        let query_count = count;
        let count = count.max(1) as u32;
        let query_set = device.wgpu_device().create_query_set(&QuerySetDescriptor {
            label: Some("moonly_light_occlusion"),
            ty: QueryType::Occlusion,
            count,
        });
        let readback = [
            create_readback(device, count),
            create_readback(device, count),
        ];
        let (sender, receiver) = channel();
        Self {
            query_set,
            readback,
            current_keys: Mutex::new(Vec::new()),
            scheduled_keys: [Mutex::new(Vec::new()), Mutex::new(Vec::new())],
            pending: AtomicBool::new(false),
            slot: AtomicUsize::new(0),
            query_count,
            visible: RwLock::new(Vec::new()),
            sender,
            receiver: Mutex::new(receiver),
        }
    }

    pub(crate) fn resize_if_needed(&mut self, device: &RenderDevice, count: usize) {
        if self.query_count != count {
            *self = Self::new(device, count);
        }
    }

    pub(crate) fn collect_results(&self) {
        let Ok(receiver) = self.receiver.lock() else {
            return;
        };
        while let Ok((slot, samples)) = receiver.try_recv() {
            self.pending.store(false, Ordering::Release);
            let Some(keys) = self.scheduled_keys.get(slot) else {
                continue;
            };
            let (Ok(keys), Ok(mut visible)) = (keys.lock(), self.visible.write()) else {
                continue;
            };
            apply_samples(&mut visible, &keys, &samples);
        }
    }

    pub(crate) fn update_keys(&self, keys: impl Iterator<Item = MoonlyLightKey>) {
        if let Ok(mut current) = self.current_keys.lock() {
            current.clear();
            current.extend(keys);
        }
    }

    pub(crate) fn is_visible(&self, key: MoonlyLightKey) -> bool {
        self.visible
            .read()
            .ok()
            .and_then(|visible| {
                visible
                    .iter()
                    .find(|(candidate, _)| *candidate == key)
                    .map(|(_, visible)| *visible)
            })
            .unwrap_or(true)
    }

    pub(crate) fn resolve_and_schedule(&self, encoder: &mut CommandEncoder) {
        let slot = self.slot.fetch_add(1, Ordering::Relaxed) % 2;
        let (Some(readback), Some(scheduled)) =
            (self.readback.get(slot), self.scheduled_keys.get(slot))
        else {
            return;
        };
        if self.pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let (Ok(keys), Ok(mut scheduled)) = (self.current_keys.lock(), scheduled.lock()) else {
            self.pending.store(false, Ordering::Release);
            return;
        };
        scheduled.clear();
        scheduled.extend_from_slice(&keys);
        encoder.resolve_query_set(&self.query_set, 0..keys.len() as u32, readback, 0);
        let sender = self.sender.clone();
        let buffer = readback.clone();
        encoder.map_buffer_on_submit(readback, MapMode::Read, .., move |result| {
            let samples = if result.is_ok() {
                let samples = buffer
                    .slice(..)
                    .get_mapped_range()
                    .map(|data| decode_samples(&data))
                    .unwrap_or_default();
                buffer.unmap();
                samples
            } else {
                Vec::new()
            };
            let _ = sender.send((slot, samples));
        });
    }
}

fn create_readback(device: &RenderDevice, count: u32) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("moonly_light_occlusion_readback"),
        size: u64::from(count) * size_of::<u64>() as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST | BufferUsages::QUERY_RESOLVE,
        mapped_at_creation: false,
    })
}

fn decode_samples(data: &[u8]) -> Vec<u64> {
    data.as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| u64::from_le_bytes(*chunk))
        .collect()
}

fn apply_samples(
    visible: &mut Vec<(MoonlyLightKey, bool)>,
    keys: &[MoonlyLightKey],
    samples: &[u64],
) {
    for (key, samples) in keys.iter().zip(samples) {
        let value = *samples > 0;
        if let Some((_, visible)) = visible.iter_mut().find(|(candidate, _)| candidate == key) {
            *visible = value;
        } else {
            visible.push((*key, value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_samples_hide_only_matching_lights_after_reordering() {
        let mut world = World::new();
        let first = MoonlyLightKey {
            source_entity: Some(world.spawn_empty().id()),
            omnipart_face: None,
        };
        let second = MoonlyLightKey {
            source_entity: Some(world.spawn_empty().id()),
            omnipart_face: Some(3),
        };
        let mut visible = Vec::new();
        apply_samples(&mut visible, &[first, second], &[1, 0]);
        apply_samples(&mut visible, &[second, first], &[2, 0]);
        assert_eq!(visible, vec![(first, false), (second, true)]);
    }
}

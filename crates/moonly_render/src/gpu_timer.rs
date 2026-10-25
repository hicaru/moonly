use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

use bevy::{
    prelude::*,
    render::{
        render_resource::{Buffer, BufferDescriptor, BufferUsages, CommandEncoder, MapMode},
        renderer::{RenderContext, RenderDevice, RenderQueue},
    },
};
use wgpu::{Features, QuerySet, QuerySetDescriptor, QueryType, RenderPassTimestampWrites};

use crate::diagnostics::{GpuTiming, MoonlyGpuDiagnostics, MoonlyPass};

const MAX_TIMED_PASSES: usize = 32;
const QUERY_COUNT: u32 = MAX_TIMED_PASSES as u32 * 2;
type GpuTimingBatch = [Option<Duration>; MoonlyPass::ALL.len()];

#[derive(Resource, Clone)]
pub(crate) struct GpuTimingSender(Sender<GpuTimingBatch>);

#[derive(Resource)]
struct GpuTimingReceiver(Mutex<Receiver<GpuTimingBatch>>);

#[derive(Resource)]
pub(crate) struct MoonlyGpuTimer {
    query_set: Option<QuerySet>,
    readback: [Buffer; 2],
    next: AtomicU32,
    slot: AtomicUsize,
    pending: [Arc<AtomicBool>; 2],
    labels: [Mutex<[MoonlyPass; MAX_TIMED_PASSES]>; 2],
    period_ns: f32,
    sender: Sender<GpuTimingBatch>,
}

impl MoonlyGpuTimer {
    fn new(device: &RenderDevice, queue: &RenderQueue, sender: Sender<GpuTimingBatch>) -> Self {
        let supported = device.features().contains(Features::TIMESTAMP_QUERY);
        let query_set = supported.then(|| {
            device.wgpu_device().create_query_set(&QuerySetDescriptor {
                label: Some("moonly_gpu_timestamps"),
                ty: QueryType::Timestamp,
                count: QUERY_COUNT,
            })
        });
        Self {
            query_set,
            readback: [create_buffer(device), create_buffer(device)],
            next: AtomicU32::new(0),
            slot: AtomicUsize::new(0),
            pending: [
                Arc::new(AtomicBool::new(false)),
                Arc::new(AtomicBool::new(false)),
            ],
            labels: [
                Mutex::new([MoonlyPass::GBuffer; MAX_TIMED_PASSES]),
                Mutex::new([MoonlyPass::GBuffer; MAX_TIMED_PASSES]),
            ],
            period_ns: queue.get_timestamp_period(),
            sender,
        }
    }

    pub(crate) fn timestamp_writes(
        &self,
        pass: MoonlyPass,
    ) -> Option<RenderPassTimestampWrites<'_>> {
        let slot = self.slot.load(Ordering::Acquire) % 2;
        if self.pending.get(slot)?.load(Ordering::Acquire) {
            return None;
        }
        let index = self.next.fetch_add(1, Ordering::AcqRel) as usize;
        if index >= MAX_TIMED_PASSES {
            self.next.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        let slot = self.slot.load(Ordering::Acquire) % 2;
        let labels = self.labels.get(slot)?;
        let Ok(mut labels) = labels.lock() else {
            return None;
        };
        *labels.get_mut(index)? = pass;
        Some(RenderPassTimestampWrites {
            query_set: self.query_set.as_ref()?,
            beginning_of_pass_write_index: Some(index as u32 * 2),
            end_of_pass_write_index: Some(index as u32 * 2 + 1),
        })
    }

    fn resolve_and_schedule(&self, encoder: &mut CommandEncoder) {
        let count = (self.next.load(Ordering::Acquire) as usize).min(MAX_TIMED_PASSES);
        if count == 0 {
            return;
        }
        let slot = self.slot.fetch_add(1, Ordering::AcqRel) % 2;
        let Some(pending) = self.pending.get(slot).cloned() else {
            return;
        };
        if pending.swap(true, Ordering::AcqRel) {
            return;
        }
        self.next.store(0, Ordering::Release);
        let (Some(query_set), Some(buffer), Some(labels)) = (
            self.query_set.as_ref(),
            self.readback.get(slot),
            self.labels.get(slot),
        ) else {
            pending.store(false, Ordering::Release);
            return;
        };
        let Ok(labels) = labels.lock().map(|labels| *labels) else {
            pending.store(false, Ordering::Release);
            return;
        };
        encoder.resolve_query_set(query_set, 0..count as u32 * 2, buffer, 0);
        let buffer = buffer.clone();
        let mapped = buffer.clone();
        let sender = self.sender.clone();
        let period_ns = self.period_ns;
        encoder.map_buffer_on_submit(&mapped, MapMode::Read, .., move |result| {
            pending.store(false, Ordering::Release);
            if result.is_ok() {
                let timings = buffer
                    .slice(..)
                    .get_mapped_range()
                    .ok()
                    .map_or([None; MoonlyPass::ALL.len()], |data| {
                        decode_timings(&data, &labels, count, period_ns)
                    });
                let _ = sender.send(timings);
                buffer.unmap();
            }
        });
    }
}

fn create_buffer(device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("moonly_gpu_timestamp_readback"),
        size: u64::from(QUERY_COUNT) * 8,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST | BufferUsages::QUERY_RESOLVE,
        mapped_at_creation: false,
    })
}

fn decode_timings(
    data: &[u8],
    labels: &[MoonlyPass; MAX_TIMED_PASSES],
    count: usize,
    period_ns: f32,
) -> GpuTimingBatch {
    let ticks = data.as_chunks::<8>().0;
    let mut batch = [None; MoonlyPass::ALL.len()];
    for (pass, label) in labels.iter().take(count).enumerate() {
        let (Some(begin), Some(end)) = (
            ticks.get(pass * 2).map(|bytes| u64::from_le_bytes(*bytes)),
            ticks
                .get(pass * 2 + 1)
                .map(|bytes| u64::from_le_bytes(*bytes)),
        ) else {
            continue;
        };
        let elapsed = end.saturating_sub(begin) as f32 * period_ns;
        let duration = Duration::from_nanos(elapsed.max(0.0) as u64);
        if let Some(slot) = batch.get_mut(label.index()) {
            *slot = Some(slot.map_or(duration, |total| total + duration));
        }
    }
    batch
}

pub(crate) fn install_gpu_timing_channel(app: &mut App) -> GpuTimingSender {
    let (sender, receiver) = channel();
    app.insert_resource(GpuTimingReceiver(Mutex::new(receiver)))
        .add_systems(PreUpdate, collect_gpu_timings);
    GpuTimingSender(sender)
}

pub(crate) fn init_gpu_timer(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    sender: Res<GpuTimingSender>,
) {
    commands.insert_resource(MoonlyGpuTimer::new(&device, &queue, sender.0.clone()));
}

pub(crate) fn resolve_gpu_timings(timer: Res<MoonlyGpuTimer>, mut context: RenderContext) {
    timer.resolve_and_schedule(context.command_encoder());
}

fn collect_gpu_timings(
    receiver: Res<GpuTimingReceiver>,
    mut diagnostics: ResMut<MoonlyGpuDiagnostics>,
) {
    let Ok(receiver) = receiver.0.lock() else {
        return;
    };
    while let Ok(batch) = receiver.try_recv() {
        for (pass, duration) in MoonlyPass::ALL.iter().zip(batch) {
            if let Some(duration) = duration {
                diagnostics.set_timing(*pass, GpuTiming::Available(duration));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_labels_are_aggregated_without_allocations() {
        let mut labels = [MoonlyPass::GBuffer; MAX_TIMED_PASSES];
        if let Some(label) = labels.get_mut(1) {
            *label = MoonlyPass::Post;
        }
        if let Some(label) = labels.get_mut(0) {
            *label = MoonlyPass::Post;
        }
        let mut data = Vec::new();
        for ticks in [10_u64, 20, 30, 50] {
            data.extend_from_slice(&ticks.to_le_bytes());
        }
        let batch = decode_timings(&data, &labels, 2, 100.0);
        assert_eq!(
            batch.get(MoonlyPass::Post.index()),
            Some(&Some(Duration::from_nanos(3000)))
        );
    }
}

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc::{Receiver, Sender, channel},
};

use bevy::{
    prelude::*,
    render::{
        render_resource::{
            Buffer, BufferDescriptor, BufferUsages, CommandEncoder, Extent3d, MapMode,
        },
        renderer::RenderDevice,
        texture::ColorAttachment,
    },
};
use moonly_post::LuminanceAdaptationState;

#[derive(Resource, Clone)]
pub(crate) struct LuminanceResultSender(pub Sender<f32>);

#[derive(Resource)]
pub(crate) struct LuminanceResultReceiver(pub Mutex<Receiver<f32>>);

pub(crate) fn install_luminance_channel(app: &mut App) -> LuminanceResultSender {
    let (sender, receiver) = channel();
    app.insert_resource(LuminanceResultReceiver(Mutex::new(receiver)))
        .add_systems(PreUpdate, collect_luminance_results);
    LuminanceResultSender(sender)
}

pub(crate) struct LuminanceReadback {
    buffers: [Buffer; 2],
    pending: Arc<AtomicBool>,
    slot: AtomicUsize,
    sender: Sender<f32>,
}

impl LuminanceReadback {
    fn new(device: &RenderDevice, sender: Sender<f32>) -> Self {
        Self {
            buffers: [create_buffer(device), create_buffer(device)],
            pending: Arc::new(AtomicBool::new(false)),
            slot: AtomicUsize::new(0),
            sender,
        }
    }

    fn schedule(&self, encoder: &mut CommandEncoder, target: &ColorAttachment) {
        if self.pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let slot = self.slot.fetch_add(1, Ordering::Relaxed) % 2;
        let Some(buffer) = self.buffers.get(slot) else {
            self.pending.store(false, Ordering::Release);
            return;
        };
        encoder.copy_texture_to_buffer(
            target.texture.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let buffer = buffer.clone();
        let sender = self.sender.clone();
        let pending = self.pending.clone();
        let mapped = buffer.clone();
        encoder.map_buffer_on_submit(&mapped, MapMode::Read, .., move |result| {
            pending.store(false, Ordering::Release);
            if result.is_ok() {
                let value = buffer.slice(..).get_mapped_range().ok().and_then(|data| {
                    data.as_chunks::<4>()
                        .0
                        .first()
                        .map(|bytes| f32::from_le_bytes(*bytes))
                });
                if let Some(value) = value.filter(|value| value.is_finite() && *value > 0.0) {
                    let _ = sender.send(value);
                }
                buffer.unmap();
            }
        });
    }
}

fn create_buffer(device: &RenderDevice) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("moonly_luminance_readback"),
        size: 256,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

pub(crate) fn create_readback(
    device: &RenderDevice,
    sender: &LuminanceResultSender,
) -> LuminanceReadback {
    LuminanceReadback::new(device, sender.0.clone())
}

pub(crate) fn schedule_readback(
    readback: &LuminanceReadback,
    encoder: &mut CommandEncoder,
    target: &ColorAttachment,
) {
    readback.schedule(encoder, target);
}

fn collect_luminance_results(
    receiver: Res<LuminanceResultReceiver>,
    mut adaptation: ResMut<LuminanceAdaptationState>,
) {
    let Ok(receiver) = receiver.0.lock() else {
        return;
    };
    while let Ok(luminance) = receiver.try_recv() {
        adaptation.target_luminance = luminance.clamp(0.0001, 100.0);
    }
}

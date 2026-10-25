use bevy::{
    prelude::*,
    render::{
        render_resource::{
            AddressMode, Extent3d, FilterMode, Sampler, SamplerDescriptor, TexelCopyBufferLayout,
            TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
            TextureViewDescriptor, TextureViewDimension,
        },
        renderer::{RenderDevice, RenderQueue},
    },
};

pub(crate) const MOONLY_JITTER_SIZE: u32 = 64;
pub(crate) const MOONLY_JITTER_LAYERS: u32 = 4;
const MOONLY_JITTER_SAMPLE_COUNT: usize = 8;
const MOONLY_JITTER_MIN_DISTANCE: i32 = 32;
const JITTER_SEED: u32 = 0x4d4f_4f4e;

#[derive(Resource)]
pub(crate) struct MoonlyJitterTexture {
    pub view: TextureView,
    pub sampler: Sampler,
}

pub(crate) fn init_jitter_texture(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let size = Extent3d {
        width: MOONLY_JITTER_SIZE,
        height: MOONLY_JITTER_SIZE,
        depth_or_array_layers: MOONLY_JITTER_LAYERS,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("moonly_moonly_jitter"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Snorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &generate_moonly_jitter(),
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(MOONLY_JITTER_SIZE * 4),
            rows_per_image: Some(MOONLY_JITTER_SIZE),
        },
        size,
    );
    let view = texture.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    });
    let sampler = device.create_sampler(&SamplerDescriptor {
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        mag_filter: FilterMode::Nearest,
        min_filter: FilterMode::Nearest,
        ..Default::default()
    });
    commands.insert_resource(MoonlyJitterTexture { view, sampler });
}

fn generate_moonly_jitter() -> Vec<u8> {
    let mut rng = JitterRng(JITTER_SEED);
    let mut output = Vec::with_capacity((MOONLY_JITTER_SIZE * MOONLY_JITTER_SIZE * 16) as usize);
    for _ in 0..MOONLY_JITTER_SIZE * MOONLY_JITTER_SIZE {
        let samples = generate_samples(&mut rng);
        for layer in 0..MOONLY_JITTER_LAYERS as usize {
            let Some(first) = samples.get(layer * 2) else {
                continue;
            };
            let Some(second) = samples.get(layer * 2 + 1) else {
                continue;
            };
            for value in [first.0, first.1, second.1, second.0] {
                output.push(value as i8 as u8);
            }
        }
    }
    output
}

fn generate_samples(rng: &mut JitterRng) -> [(u8, u8); MOONLY_JITTER_SAMPLE_COUNT] {
    let mut samples = [(0, 0); MOONLY_JITTER_SAMPLE_COUNT];
    let mut count = 0;
    while count < MOONLY_JITTER_SAMPLE_COUNT {
        let candidate = (rng.next_byte(), rng.next_byte());
        let valid = samples.iter().take(count).all(|sample| {
            let distance = (i32::from(candidate.0) - i32::from(sample.0)).abs()
                + (i32::from(candidate.1) - i32::from(sample.1)).abs();
            distance >= MOONLY_JITTER_MIN_DISTANCE
        });
        if valid && let Some(slot) = samples.get_mut(count) {
            *slot = candidate;
            count += 1;
        }
    }
    samples
}

struct JitterRng(u32);

impl JitterRng {
    const fn next_byte(&mut self) -> u8 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 & 0xff) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_has_original_shape_and_sample_spacing() {
        let data = generate_moonly_jitter();
        assert_eq!(
            data.len(),
            (MOONLY_JITTER_SIZE * MOONLY_JITTER_SIZE * 16) as usize
        );
        let mut rng = JitterRng(JITTER_SEED);
        let samples = generate_samples(&mut rng);
        for (index, first) in samples.iter().enumerate() {
            for second in samples.iter().skip(index + 1) {
                let distance = (i32::from(first.0) - i32::from(second.0)).abs()
                    + (i32::from(first.1) - i32::from(second.1)).abs();
                assert!(distance >= MOONLY_JITTER_MIN_DISTANCE);
            }
        }
    }
}

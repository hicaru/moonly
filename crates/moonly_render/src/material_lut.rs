use bevy::{
    prelude::*,
    render::{
        render_resource::{
            Extent3d, Sampler, SamplerDescriptor, TexelCopyBufferLayout, TextureDescriptor,
            TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
        },
        renderer::{RenderDevice, RenderQueue},
    },
};

pub const MOONLY_MATERIAL_LDOTN: usize = 128;
pub const MOONLY_MATERIAL_LDOTH: usize = 256;
pub const MOONLY_MATERIAL_SLICES: usize = 4;
pub const MOONLY_MATERIAL_LUT_BYTES: usize =
    MOONLY_MATERIAL_LDOTN * MOONLY_MATERIAL_LDOTH * MOONLY_MATERIAL_SLICES * 2;
const EPS_S: f32 = 0.000_000_1;

/// Four `R8G8` material response slices from Moonly R4.
#[derive(Resource, Debug, Clone)]
pub struct MoonlyMaterialLut {
    data: Vec<u8>,
}

impl Default for MoonlyMaterialLut {
    fn default() -> Self {
        Self {
            data: generate_moonly_material_lut(),
        }
    }
}

impl MoonlyMaterialLut {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    #[must_use]
    pub fn sample(&self, slice: usize, x: usize, y: usize) -> Option<[u8; 2]> {
        if slice >= MOONLY_MATERIAL_SLICES
            || x >= MOONLY_MATERIAL_LDOTN
            || y >= MOONLY_MATERIAL_LDOTH
        {
            return None;
        }
        let index = ((slice * MOONLY_MATERIAL_LDOTH + y) * MOONLY_MATERIAL_LDOTN + x) * 2;
        Some([*self.data.get(index)?, *self.data.get(index + 1)?])
    }
}

#[derive(Resource)]
pub(crate) struct MoonlyMaterialLutTexture {
    pub view: TextureView,
    pub sampler: Sampler,
}

pub(crate) fn init_moonly_material_lut_texture(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let data = generate_moonly_material_lut();
    let size = Extent3d {
        width: MOONLY_MATERIAL_LDOTN as u32,
        height: MOONLY_MATERIAL_LDOTH as u32,
        depth_or_array_layers: MOONLY_MATERIAL_SLICES as u32,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("moonly_material_lut"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D3,
        format: TextureFormat::Rg8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &data,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some((MOONLY_MATERIAL_LDOTN * 2) as u32),
            rows_per_image: Some(MOONLY_MATERIAL_LDOTH as u32),
        },
        size,
    );
    commands.insert_resource(MoonlyMaterialLutTexture {
        view: texture.create_view(&TextureViewDescriptor::default()),
        sampler: device.create_sampler(&SamplerDescriptor::default()),
    });
}

/// Generate the immutable material table from `r4_rendertarget.cpp`.
#[must_use]
pub fn generate_moonly_material_lut() -> Vec<u8> {
    let mut data = Vec::with_capacity(MOONLY_MATERIAL_LUT_BYTES);
    for slice in 0..MOONLY_MATERIAL_SLICES {
        for y in 0..MOONLY_MATERIAL_LDOTH {
            for x in 0..MOONLY_MATERIAL_LDOTN {
                let ld = x as f32 / (MOONLY_MATERIAL_LDOTN - 1) as f32;
                let mut ls = y as f32 / (MOONLY_MATERIAL_LDOTH - 1) as f32 + EPS_S;
                ls *= ld.powf(1.0 / 32.0);
                let (fd, fs) = material_response(slice, ld, ls);
                data.push(quantize(fd));
                data.push(quantize(fs));
            }
        }
    }
    for slice in 0..MOONLY_MATERIAL_SLICES {
        let index = ((slice * MOONLY_MATERIAL_LDOTH + MOONLY_MATERIAL_LDOTH - 1)
            * MOONLY_MATERIAL_LDOTN
            + MOONLY_MATERIAL_LDOTN
            - 1)
            * 2;
        if let Some(diffuse) = data.get_mut(index) {
            *diffuse = 255;
        }
        if let Some(specular) = data.get_mut(index + 1) {
            *specular = 255;
        }
    }
    data
}

fn material_response(slice: usize, ld: f32, ls: f32) -> (f32, f32) {
    match slice {
        0 => (ld.powf(0.75), ls.powf(16.0) * 0.5),
        1 => (ld.powf(0.90), ls.powf(24.0)),
        2 => (ld, (ls * 1.01).powf(128.0)),
        3 => {
            let s0 = (1.0 - (0.05 * (33.0 * ld).sin() + ld - ls).abs()).abs();
            let s1 = (1.0 - (0.05 * (33.0 * ld * ls).cos() + ld - ls).abs()).abs();
            let s2 = (1.0 - (ld - ls).abs()).abs();
            (ld, s0.max(s1).max(s2).powf(24.0) * ld.powf(1.0 / 7.0))
        }
        _ => (0.0, 0.0),
    }
}

fn quantize(value: f32) -> u8 {
    (value * 255.5).floor().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_has_four_r8g8_slices_and_forced_white_corners() {
        let lut = MoonlyMaterialLut::default();
        assert_eq!(lut.bytes().len(), MOONLY_MATERIAL_LUT_BYTES);
        for slice in 0..MOONLY_MATERIAL_SLICES {
            assert_eq!(
                lut.sample(slice, MOONLY_MATERIAL_LDOTN - 1, MOONLY_MATERIAL_LDOTH - 1),
                Some([255, 255])
            );
        }
    }

    #[test]
    fn lut_preserves_response_exponents_and_zero_specular() {
        let lut = MoonlyMaterialLut::default();
        assert_eq!(lut.sample(0, MOONLY_MATERIAL_LDOTN - 1, 0), Some([255, 0]));
        assert_eq!(lut.sample(1, MOONLY_MATERIAL_LDOTN - 1, 0), Some([255, 0]));
        assert_eq!(lut.sample(2, 0, 0), Some([0, 0]));
        assert_eq!(lut.sample(3, 0, 0), Some([0, 0]));
    }
}

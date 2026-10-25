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

#[derive(Resource)]
pub(crate) struct MoonlyFallbackProjector {
    pub view: TextureView,
    pub sampler: Sampler,
}

pub(crate) fn init_fallback_projector(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let size = Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("moonly_fallback_projector"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &[255, 255, 255, 255],
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        size,
    );
    commands.insert_resource(MoonlyFallbackProjector {
        view: texture.create_view(&TextureViewDescriptor::default()),
        sampler: device.create_sampler(&SamplerDescriptor::default()),
    });
}

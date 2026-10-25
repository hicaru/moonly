use bevy::{
    color::LinearRgba,
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::RenderDevice,
        texture::{ColorAttachment, DepthStencilAttachment, TextureCache},
    },
};

use crate::camera::MoonlyCamera;

pub(crate) const GBUFFER_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
pub(crate) const GBUFFER_DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32FloatStencil8;
pub(crate) const DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

#[derive(Component)]
pub(crate) struct MoonlyViewTargets {
    pub position: ColorAttachment,
    pub normal: ColorAttachment,
    pub albedo_gloss: ColorAttachment,
    pub accumulator: ColorAttachment,
    pub scene: ColorAttachment,
    pub volumetrics: ColorAttachment,
    pub local_volumetrics: ColorAttachment,
    pub froxel: ColorAttachment,
    pub bloom: [ColorAttachment; 3],
    pub ao: ColorAttachment,
    pub depth: DepthStencilAttachment,
}

pub(crate) fn prepare_view_targets(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    views: Query<(Entity, &ExtractedCamera), With<MoonlyCamera>>,
) {
    for (entity, camera) in &views {
        let Some(size) = camera.physical_viewport_size else {
            commands.entity(entity).remove::<MoonlyViewTargets>();
            continue;
        };
        if size.x == 0 || size.y == 0 {
            commands.entity(entity).remove::<MoonlyViewTargets>();
            continue;
        }
        let targets = create_targets(&render_device, &mut texture_cache, size);
        commands.entity(entity).insert(targets);
    }
}

fn create_targets(
    device: &RenderDevice,
    cache: &mut TextureCache,
    size: UVec2,
) -> MoonlyViewTargets {
    let position = color_target(device, cache, size, "moonly_position");
    let normal = color_target(device, cache, size, "moonly_normal");
    let albedo_gloss = color_target(device, cache, size, "moonly_albedo_gloss");
    let accumulator = color_target(device, cache, size, "moonly_accumulator");
    let scene = color_target(device, cache, size, "moonly_scene");
    let volumetrics = color_target(device, cache, size, "moonly_volumetrics");
    let local_volumetrics = color_target(
        device,
        cache,
        (size / 8).max(UVec2::ONE),
        "moonly_local_volumetrics",
    );
    let froxel_texture = cache.get(
        device,
        texture_descriptor((size / 4).max(UVec2::ONE), GBUFFER_FORMAT, "moonly_froxel"),
    );
    let froxel = ColorAttachment::new(
        froxel_texture,
        None,
        None,
        Some(LinearRgba::new(0.0, 0.0, 0.0, 1.0).into()),
    );
    let bloom = [2, 4, 8].map(|scale| {
        color_target(
            device,
            cache,
            (size / scale).max(UVec2::ONE),
            "moonly_bloom",
        )
    });
    let ao_texture = cache.get(
        device,
        texture_descriptor(size, GBUFFER_FORMAT, "moonly_ao"),
    );
    let ao = ColorAttachment::new(ao_texture, None, None, Some(LinearRgba::WHITE.into()));
    let depth_texture = cache.get(
        device,
        texture_descriptor(size, GBUFFER_DEPTH_FORMAT, "moonly_depth"),
    );
    let depth = DepthStencilAttachment::new(depth_texture, None, Some(0.0), Some(0));
    MoonlyViewTargets {
        position,
        normal,
        albedo_gloss,
        accumulator,
        scene,
        volumetrics,
        local_volumetrics,
        froxel,
        bloom,
        ao,
        depth,
    }
}

fn color_target(
    device: &RenderDevice,
    cache: &mut TextureCache,
    size: UVec2,
    label: &'static str,
) -> ColorAttachment {
    let texture = cache.get(device, texture_descriptor(size, GBUFFER_FORMAT, label));
    ColorAttachment::new(texture, None, None, Some(LinearRgba::NONE.into()))
}

fn texture_descriptor(
    size: UVec2,
    format: TextureFormat,
    label: &'static str,
) -> TextureDescriptor<'static> {
    TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    }
}

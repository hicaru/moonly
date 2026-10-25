use bevy::{
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    image::ToExtents,
    pbr::{ScreenSpaceTransmission, Transmissive3d, ViewTransmissionTexture},
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_phase::{ViewBinnedRenderPhases, ViewSortedRenderPhases},
        render_resource::{
            FilterMode, SamplerDescriptor, TextureDescriptor, TextureDimension, TextureUsages,
        },
        renderer::RenderDevice,
        texture::TextureCache,
        view::ExtractedView,
    },
};

use crate::camera::MoonlyCamera;

type TransmissionViewQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ExtractedCamera,
        &'static ScreenSpaceTransmission,
        &'static ExtractedView,
    ),
    With<MoonlyCamera>,
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct TransmissionPrepareInputs<'w> {
    device: Res<'w, RenderDevice>,
    cache: ResMut<'w, TextureCache>,
    opaque: Res<'w, ViewBinnedRenderPhases<Opaque3d>>,
    alpha: Res<'w, ViewBinnedRenderPhases<AlphaMask3d>>,
    transparent: Res<'w, ViewSortedRenderPhases<Transparent3d>>,
    transmissive: Res<'w, ViewSortedRenderPhases<Transmissive3d>>,
}

pub(crate) fn prepare_transmission_textures(
    mut commands: Commands,
    mut inputs: TransmissionPrepareInputs,
    views: TransmissionViewQuery,
) {
    for (entity, camera, settings, view) in &views {
        let ready = inputs.opaque.contains_key(&view.retained_view_entity)
            && inputs.alpha.contains_key(&view.retained_view_entity)
            && inputs.transparent.contains_key(&view.retained_view_entity);
        let has_items = inputs
            .transmissive
            .get(&view.retained_view_entity)
            .is_some_and(|phase| !phase.items.is_empty());
        let Some(size) = camera.physical_target_size else {
            continue;
        };
        if !ready || !has_items || settings.steps == 0 {
            continue;
        }
        let cached = inputs.cache.get(
            &inputs.device,
            TextureDescriptor {
                label: Some("moonly_transmission_texture"),
                size: size.to_extents(),
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: view.target_format,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            },
        );
        let sampler = inputs.device.create_sampler(&SamplerDescriptor {
            label: Some("moonly_transmission_sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        });
        commands.entity(entity).insert(ViewTransmissionTexture {
            texture: cached.texture,
            view: cached.default_view,
            sampler,
        });
    }
}

use std::sync::atomic::{AtomicUsize, Ordering};

use bevy::{
    prelude::*,
    render::{
        render_resource::{BindGroup, TextureView},
        texture::{ColorAttachment, DepthStencilAttachment},
    },
};
use moonly_core::{MoonlyLight, MoonlyShadowRect, SpotShadowXform};

use crate::minmax::MoonlyMinMaxTarget;

pub(crate) struct MoonlyShadowViewTarget {
    pub source_entity: Option<Entity>,
    pub omnipart_face: Option<u8>,
    pub xform: SpotShadowXform,
    pub rect: MoonlyShadowRect,
    pub bind_group: BindGroup,
}

pub(crate) struct MoonlyShadowBatchTarget {
    pub depth: DepthStencilAttachment,
    pub transmittance: Option<ColorAttachment>,
    pub views: Vec<MoonlyShadowViewTarget>,
}

#[derive(Component)]
pub(crate) struct MoonlySpotShadowTarget {
    pub batches: Vec<MoonlyShadowBatchTarget>,
    pub sun_batches: Vec<MoonlyShadowBatchTarget>,
    pub near_minmax: Option<MoonlyMinMaxTarget>,
    pub atlas_size: u32,
    pub fallback_xform: SpotShadowXform,
}

#[derive(Component, Default)]
pub(crate) struct MoonlyShadowViewIndex {
    pub domain: AtomicUsize,
    pub batch: AtomicUsize,
    pub view: AtomicUsize,
}

pub(crate) struct MoonlyShadowSample<'a> {
    pub depth: &'a TextureView,
    pub transmittance: Option<&'a TextureView>,
    pub xform: &'a SpotShadowXform,
    pub rect: MoonlyShadowRect,
    pub atlas_size: u32,
    pub enabled: bool,
}

impl MoonlySpotShadowTarget {
    pub(crate) fn active_view(
        &self,
        index: &MoonlyShadowViewIndex,
    ) -> Option<&MoonlyShadowViewTarget> {
        let batches = if index.domain.load(Ordering::Relaxed) == 0 {
            &self.batches
        } else {
            &self.sun_batches
        };
        batches
            .get(index.batch.load(Ordering::Relaxed))
            .and_then(|batch| batch.views.get(index.view.load(Ordering::Relaxed)))
    }

    pub(crate) fn sample(&self, light: &MoonlyLight) -> Option<MoonlyShadowSample<'_>> {
        let selected = light.source_entity.and_then(|source| {
            self.batches.iter().find_map(|batch| {
                batch
                    .views
                    .iter()
                    .find(|view| {
                        view.source_entity == Some(source)
                            && view.omnipart_face == light.omnipart_face
                    })
                    .map(|view| (batch, view))
            })
        });
        let batch = selected
            .map(|(batch, _)| batch)
            .or_else(|| self.batches.first())?;
        let view = selected.map(|(_, view)| view);
        Some(MoonlyShadowSample {
            depth: &batch.depth.texture.default_view,
            transmittance: batch
                .transmittance
                .as_ref()
                .map(|attachment| &attachment.texture.default_view),
            xform: view.map_or(&self.fallback_xform, |view| &view.xform),
            rect: view.map_or(
                MoonlyShadowRect {
                    x: 0,
                    y: 0,
                    size: self.atlas_size,
                },
                |view| view.rect,
            ),
            atlas_size: self.atlas_size,
            enabled: view.is_some(),
        })
    }

    pub(crate) fn first_depth(&self) -> Option<&TextureView> {
        self.batches
            .first()
            .map(|batch| &batch.depth.texture.default_view)
    }
}

use bevy::{
    prelude::*,
    render::{render_resource::ShaderType, view::ExtractedView},
};
use moonly_core::{MoonlyLight, MoonlySunCascade};

use crate::shadow_targets::MoonlySpotShadowTarget;

#[derive(ShaderType, Default)]
pub(crate) struct MoonlySunUniform {
    view_position: Vec4,
    view_forward: Vec4,
    direction_bias: Vec4,
    color: Vec4,
    splits_near: Vec4,
    splits_far: Vec4,
    cascade_biases: Vec4,
    blend_bands: Vec4,
    shadow_near_from_world: Mat4,
    shadow_middle_from_world: Mat4,
    shadow_far_from_world: Mat4,
    previous_near_from_world: Mat4,
    previous_middle_from_world: Mat4,
    previous_far_from_world: Mat4,
}

impl MoonlySunUniform {
    pub(crate) fn new(
        light: &MoonlyLight,
        view: &ExtractedView,
        cascades: &[MoonlySunCascade],
        target: &MoonlySpotShadowTarget,
    ) -> Self {
        let split_far = |index: usize| cascades.get(index).map_or(0.0, |cascade| cascade.split_far);
        let split_near = |index: usize| {
            cascades
                .get(index)
                .map_or(0.0, |cascade| cascade.split_near)
        };
        let bias = |index: usize| {
            cascades
                .get(index)
                .map_or(0.0, |cascade| cascade.depth_bias)
        };
        let band = |index: usize| {
            cascades
                .get(index)
                .map_or(0.0, |cascade| cascade.blend_band)
        };
        let previous = |index: usize| {
            cascades
                .get(index)
                .map_or(Mat4::IDENTITY, |cascade| cascade.prev_combine)
        };
        let shadow_texel = target
            .sun_batches
            .first()
            .and_then(|batch| batch.views.first())
            .map_or(0.0, |view| 1.0 / view.rect.size as f32);
        Self {
            view_position: view.world_from_view.translation().extend(shadow_texel),
            view_forward: view.world_from_view.forward().as_vec3().extend(0.0),
            direction_bias: light.direction.extend(bias(0)),
            color: light.color.extend(
                if light.flags.contains(moonly_core::MoonlyLightFlags::NATIVE) {
                    1.0
                } else {
                    0.0
                },
            ),
            splits_near: Vec4::new(split_near(0), split_near(1), split_near(2), 0.0),
            splits_far: Vec4::new(
                split_far(0),
                split_far(1),
                split_far(2),
                if target.near_minmax.is_some() {
                    1.0
                } else {
                    0.0
                },
            ),
            cascade_biases: Vec4::new(bias(0), bias(1), bias(2), 0.0),
            blend_bands: Vec4::new(band(0), band(1), band(2), 0.0),
            shadow_near_from_world: cascade_matrix(target, 0),
            shadow_middle_from_world: cascade_matrix(target, 1),
            shadow_far_from_world: cascade_matrix(target, 2),
            previous_near_from_world: previous(0),
            previous_middle_from_world: previous(1),
            previous_far_from_world: previous(2),
        }
    }
}

fn cascade_matrix(target: &MoonlySpotShadowTarget, index: usize) -> Mat4 {
    target
        .sun_batches
        .get(index)
        .and_then(|batch| batch.views.first())
        .map_or(Mat4::IDENTITY, |view| view.xform.combine)
}

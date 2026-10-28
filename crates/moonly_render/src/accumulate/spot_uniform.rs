use bevy::{
    prelude::*,
    render::{render_resource::ShaderType, view::ExtractedView},
};
use moonly_core::{
    MoonlyLight, MoonlyLightFlags, MoonlyLightType, VolumetricQualityConfig,
    attenuation::reference_attenuation_factor,
};

use crate::{light_volume::clip_from_local, shadow_targets::MoonlyShadowSample};

#[derive(ShaderType, Clone, Copy)]
pub(crate) struct MoonlySpotUniform {
    position_range: Vec4,
    direction_cos: Vec4,
    color: Vec4,
    view_position: Vec4,
    shadow_from_world: Mat4,
    projector_from_world: Mat4,
    clip_from_local: Mat4,
    shadow_settings: Vec4,
    shadow_transform: Vec4,
    world_from_clip: Mat4,
    volumetric: Vec4,
    screen_attenuation: Vec4,
    volumetric_schedule: Vec4,
}

impl MoonlySpotUniform {
    pub(crate) fn new(
        light: &MoonlyLight,
        view: &ExtractedView,
        shadow: Option<&MoonlyShadowSample>,
        has_projector: bool,
        config: &VolumetricQualityConfig,
    ) -> Self {
        let camera_position = view.world_from_view.translation();
        let (volumetric, volumetric_schedule) = volumetric_data(light, camera_position, config);
        let atlas_size = shadow.map_or(2048, |shadow| shadow.atlas_size);
        let rect = shadow.map_or(
            moonly_core::MoonlyShadowRect {
                x: 0,
                y: 0,
                size: atlas_size,
            },
            |shadow| shadow.rect,
        );
        let projector =
            moonly_core::SpotShadowXform::compute_xf_spot(light, camera_position, atlas_size, 0);
        let clip_from_world = view.clip_from_view * view.world_from_view.to_matrix().inverse();
        let lowres_size = Vec2::new(
            (view.viewport.z / 8).max(1) as f32,
            (view.viewport.w / 8).max(1) as f32,
        );
        Self {
            position_range: light.position.extend(light.range),
            direction_cos: light
                .direction
                .extend(if light.light_type == MoonlyLightType::Point {
                    -1.0
                } else {
                    (light.cone_angle * 0.5).cos()
                }),
            color: light
                .color
                .extend(if light.flags.contains(MoonlyLightFlags::NATIVE) {
                    1.0
                } else {
                    0.0
                }),
            view_position: camera_position.extend(1.0),
            shadow_from_world: shadow.map_or(Mat4::IDENTITY, |shadow| shadow.xform.combine),
            projector_from_world: projector.combine,
            clip_from_local: clip_from_local(light, view),
            shadow_settings: Vec4::new(
                if shadow.is_some_and(|shadow| shadow.enabled) {
                    1.0
                } else {
                    0.0
                },
                0.0005,
                1.0 / rect.size as f32,
                if has_projector { 1.0 } else { 0.0 },
            ),
            shadow_transform: Vec4::new(
                rect.x as f32 / atlas_size as f32,
                rect.y as f32 / atlas_size as f32,
                rect.size as f32 / atlas_size as f32,
                0.0,
            ),
            world_from_clip: clip_from_world.inverse(),
            volumetric,
            screen_attenuation: lowres_size
                .extend(reference_attenuation_factor(light.range))
                .extend(0.0),
            volumetric_schedule,
        }
    }

    pub(crate) fn has_volumetric_contribution(&self) -> bool {
        self.volumetric.x > 0.0
    }
}

fn volumetric_data(
    light: &MoonlyLight,
    camera_position: Vec3,
    config: &VolumetricQualityConfig,
) -> (Vec4, Vec4) {
    let schedule = config.schedule(
        light.volumetric.quality,
        camera_position.distance(light.position),
        light.range,
    );
    let intensity =
        light.volumetric.intensity.max(0.0) * schedule.distance_fade * config.density_scale();
    let inner_cone = if light.inner_cone_angle >= 0.0 {
        (light.inner_cone_angle * 0.5).cos()
    } else {
        -1.0
    };
    let settings = Vec4::new(
        intensity,
        light.volumetric.distance,
        light.volumetric.quality,
        inner_cone,
    );
    let schedule = Vec4::new(
        schedule.min_step,
        schedule.intensity_normalization,
        schedule.slice_count as f32,
        0.0,
    );
    (settings, schedule)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_and_live_lights_share_the_attenuation_factor() {
        let point = MoonlyLight {
            range: 10.0,
            ..MoonlyLight::default()
        };
        let native_spot = MoonlyLight {
            light_type: MoonlyLightType::Spot,
            flags: MoonlyLightFlags::NATIVE,
            range: point.range,
            ..MoonlyLight::default()
        };
        let point_factor = reference_attenuation_factor(point.range);
        let spot_factor = reference_attenuation_factor(native_spot.range);
        assert!((point_factor - spot_factor).abs() < f32::EPSILON);
        assert!((point_factor - 1.0 / 90.25).abs() < 1.0e-7);
    }
}

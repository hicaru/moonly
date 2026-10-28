//! Frame-level camera selection, cascade updates, rain shadows, and atlas allocation.

use bevy::{ecs::system::SystemParam, light::DirectionalLightShadowMap, prelude::*};
use moonly_core::{
    LightPackage, MoonlyConfig, MoonlyLightFlags, MoonlyShadowBatch, MoonlyShadowPackage,
    MoonlyShadowRect, MoonlyShadowView, MoonlySunCascade, MoonlySunPackage, WeatherEnvironment,
};

use crate::{
    CameraFrustumParams, LocalShadowBatches, RainShadowState, SmapAllocator, SpotShadowXform,
    SunCascades,
};

/// Resources and camera query used by the shadow frame update.
#[derive(SystemParam)]
pub struct ShadowFrameParams<'w, 's> {
    config: Res<'w, MoonlyConfig>,
    env: Res<'w, WeatherEnvironment>,
    directional_shadow_map: Res<'w, DirectionalLightShadowMap>,
    native_forward: Res<'w, moonly_core::MoonlyNativeForward>,
    package: Res<'w, LightPackage>,
    allocator: ResMut<'w, SmapAllocator>,
    batches: ResMut<'w, LocalShadowBatches>,
    render_package: Option<ResMut<'w, MoonlyShadowPackage>>,
    render_sun: Option<ResMut<'w, MoonlySunPackage>>,
    cascades: ResMut<'w, SunCascades>,
    rain: ResMut<'w, RainShadowState>,
    cameras: Query<
        'w,
        's,
        (
            &'static Camera,
            &'static Projection,
            &'static GlobalTransform,
        ),
    >,
}

/// Update sun shadow cascades, rain shadow state, and dynamic SMAP allocations each frame.
pub fn update_shadow_cascades_and_atlas(mut params: ShadowFrameParams) {
    if params.native_forward.0 {
        return;
    }
    let Some(cam) = active_camera_params(&params.cameras) else {
        return;
    };
    let sun_dir = params
        .package
        .v_direct
        .first()
        .map_or(params.env.sun_direction, |light| light.direction);
    let native_sun = params
        .package
        .v_direct
        .first()
        .is_some_and(|light| light.flags.contains(MoonlyLightFlags::NATIVE));
    let sun_pool_size = if native_sun {
        params
            .directional_shadow_map
            .size
            .clamp(1, u32::MAX as usize) as u32
    } else {
        params.config.smap_pool_size
    };
    params.allocator.begin_frame(params.config.smap_pool_size);
    params
        .cascades
        .compute(&cam, sun_dir, &params.config.sun_cascades, sun_pool_size);
    update_rain_shadow(&params.config, &mut params.rain, &cam);
    params.batches.rebuild(
        &params.package,
        &params.config,
        cam.camera_transform.translation,
    );
    if let Some(render_package) = params.render_package.as_mut() {
        sync_render_package(
            &params.batches,
            render_package,
            params.config.smap_pool_size,
        );
    }
    if let Some(render_sun) = params.render_sun.as_mut() {
        sync_sun_package(
            &params.cascades,
            render_sun,
            sun_pool_size,
            params.config.sun_cascades.depth_bias,
            params.config.sun_cascades.use_minmax_near,
        );
    }
}

fn sync_render_package(
    source: &LocalShadowBatches,
    target: &mut MoonlyShadowPackage,
    pool_size: u32,
) {
    target.pool_size = pool_size;
    target
        .batches
        .resize_with(source.active().len(), MoonlyShadowBatch::default);
    for (target_batch, source_batch) in target.batches.iter_mut().zip(source.active()) {
        target_batch.views.clear();
        target_batch
            .views
            .extend(source_batch.views.iter().map(|view| MoonlyShadowView {
                light_index: view.light_index,
                xform: view.xform,
                rect: MoonlyShadowRect {
                    x: view.rect.min_x,
                    y: view.rect.min_y,
                    size: view.rect.width(),
                },
            }));
    }
}

fn sync_sun_package(
    source: &SunCascades,
    target: &mut MoonlySunPackage,
    pool_size: u32,
    depth_bias: f32,
    use_minmax_near: bool,
) {
    target.pool_size = pool_size.max(1);
    target.depth_bias = depth_bias;
    target.use_minmax_near = use_minmax_near;
    target.cascades.clear();
    target
        .cascades
        .extend(
            source
                .cascades
                .iter()
                .enumerate()
                .map(|(index, cascade)| MoonlySunCascade {
                    view: MoonlyShadowView {
                        light_index: 0,
                        xform: SpotShadowXform {
                            view: cascade.view,
                            proj: cascade.proj,
                            combine: cascade.combine,
                            smap_size: target.pool_size,
                            screen_space_area: 1.0,
                        },
                        rect: MoonlyShadowRect {
                            x: 0,
                            y: 0,
                            size: target.pool_size,
                        },
                    },
                    prev_combine: crate::sun_cascades::previous_cascade_combine(
                        &source.cascades,
                        index,
                    ),
                    split_near: cascade.split_near,
                    split_far: cascade.split_far,
                    depth_bias: cascade.depth_bias,
                    blend_band: cascade.blend_band,
                }),
        );
}

fn active_camera_params(
    cameras: &Query<(&Camera, &Projection, &GlobalTransform)>,
) -> Option<CameraFrustumParams> {
    let active = cameras
        .iter()
        .filter(|(camera, _, _)| camera.is_active)
        .max_by_key(|(camera, _, _)| camera.order);
    let (_, projection, transform) = active?;
    let Projection::Perspective(perspective) = projection else {
        warn!("Moonly sun cascades require a perspective camera");
        return None;
    };
    Some(CameraFrustumParams {
        camera_transform: Transform {
            translation: transform.translation(),
            rotation: transform.rotation(),
            scale: transform.scale(),
        },
        fov_y_radians: perspective.fov,
        aspect_ratio: perspective.aspect_ratio,
        z_near: perspective.near,
    })
}

fn update_rain_shadow(
    config: &MoonlyConfig,
    rain: &mut RainShadowState,
    cam: &CameraFrustumParams,
) {
    if config.rain_wetness <= 0.0 {
        return;
    }
    *rain = RainShadowState::compute(
        cam.camera_transform.translation,
        cam.camera_transform.forward().as_vec3(),
        rain.rain_direction,
        cam.fov_y_radians,
        cam.aspect_ratio,
    );
}

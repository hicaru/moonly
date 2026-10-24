//! Frame-level camera selection, cascade updates, rain shadows, and atlas allocation.

use bevy::{ecs::system::SystemParam, prelude::*};
use moonly_core::{LightPackage, MoonlyConfig, MoonlyPipelineTier, WeatherEnvironment};

use crate::{CameraFrustumParams, RainShadowState, SmapAllocator, SpotShadowXform, SunCascades};

/// Resources and camera query used by the shadow frame update.
#[derive(SystemParam)]
pub struct ShadowFrameParams<'w, 's> {
    config: Res<'w, MoonlyConfig>,
    env: Res<'w, WeatherEnvironment>,
    package: Res<'w, LightPackage>,
    allocator: ResMut<'w, SmapAllocator>,
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
    let Some(cam) = active_camera_params(&params.cameras) else {
        return;
    };
    let sun_dir = params
        .package
        .v_direct
        .first()
        .map_or(params.env.sun_direction, |light| light.direction);
    params.allocator.begin_frame(params.config.smap_pool_size);
    params.cascades.compute(
        &cam,
        sun_dir,
        &params.config.sun_cascades,
        params.config.smap_pool_size,
    );
    update_rain_shadow(&params.config, &mut params.rain, &cam);
    allocate_spot_shadows(
        &params.config,
        &params.package,
        cam.camera_transform.translation,
        &mut params.allocator,
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

fn allocate_spot_shadows(
    config: &MoonlyConfig,
    package: &LightPackage,
    camera_pos: Vec3,
    allocator: &mut SmapAllocator,
) {
    for (index, light) in package.v_shadowed.iter().enumerate() {
        let previous_size = allocator.previous_smap_size(index);
        let xf = SpotShadowXform::compute_xf_spot(
            light,
            camera_pos,
            config.smap_pool_size,
            previous_size,
        );
        allocator.record_smap_size(index, xf.smap_size);
        let allocated = if matches!(config.tier, MoonlyPipelineTier::MoonlyEnhanced) {
            allocator.push_enhanced_bssf(xf.smap_size)
        } else {
            allocator.push_monolith(xf.smap_size)
        };
        if allocated.is_none() {
            warn!(
                "Shadow atlas allocation failed for light {} at size {}",
                index, xf.smap_size
            );
        }
    }
    allocator.finish_frame(package.v_shadowed.len());
}

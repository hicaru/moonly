use bevy::{
    camera::{Exposure, RenderTarget, visibility::RenderLayers},
    pbr::DistanceFog,
    prelude::*,
    render::{camera::CameraRenderGraph, render_resource::TextureFormat},
};
use moonly_core::{MoonlyLight, MoonlyLightFlags, MoonlyLightType};
use moonly_gi::MoonlyGlow;
use moonly_render::{MoonlyCamera, MoonlyCameraRole, MoonlyLinearOutput, MoonlyRender};

use super::{
    CAPTURE_HEIGHT, CAPTURE_WIDTH,
    capture::{CaptureOptions, OffscreenCapture},
    reference_scene::debug_view_from_args,
};

pub(crate) fn spawn_native_spot(commands: &mut Commands) {
    commands.spawn((
        SpotLight {
            color: Color::linear_rgb(1.0, 0.75, 0.46875),
            intensity: 12.8 * 4.0 * std::f32::consts::PI,
            range: 18.0,
            inner_angle: 0.9,
            outer_angle: 1.4,
            shadow_maps_enabled: true,
            ..SpotLight::default()
        },
        Transform::from_xyz(-2.0, 5.0, 3.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
}

pub(crate) fn setup_offscreen_capture(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut options: ResMut<CaptureOptions>,
) {
    if !std::env::args().any(|arg| arg == "--offscreen") {
        return;
    }
    let image = images.add(Image::new_target_texture(
        CAPTURE_WIDTH,
        CAPTURE_HEIGHT,
        TextureFormat::Rgba16Float,
        None,
    ));
    options.offscreen = Some(OffscreenCapture(image.clone()));
    let mut camera = commands.spawn((
        MoonlyCamera::default(),
        debug_view_from_args(),
        Camera::default(),
        Exposure { ev100: 0.0 },
        RenderTarget::Image(image.clone().into()),
        CameraRenderGraph::new(MoonlyRender),
        Transform::from_xyz(5.5, 4.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
    if std::env::args().any(|arg| arg == "--linear") {
        camera.insert(MoonlyLinearOutput);
    }
    if std::env::args().any(|arg| arg == "--fog" || arg == "--fog-transparent") {
        camera.insert(DistanceFog::default());
    }
    if std::env::args().any(|arg| arg == "--overlay") {
        commands.spawn((
            MoonlyCamera {
                role: MoonlyCameraRole::Overlay,
            },
            Camera {
                order: 1,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..Camera::default()
            },
            RenderTarget::Image(image.into()),
            RenderLayers::layer(1),
            CameraRenderGraph::new(MoonlyRender),
            Transform::from_xyz(5.5, 4.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
        ));
    }
}

pub(crate) fn spawn_sun(commands: &mut Commands) {
    if !std::env::args().any(|arg| {
        arg == "--sun" || arg == "--screen-shafts" || arg == "--cascade-motion" || arg == "--froxel"
    }) {
        return;
    }
    let screen_shafts = std::env::args().any(|arg| arg == "--screen-shafts");
    let direction = if screen_shafts {
        Vec3::new(0.65, -0.12, 0.75)
    } else {
        Vec3::new(0.4, -1.0, 0.2)
    };
    let target = if screen_shafts {
        Vec3::new(0.0, 10.0, 0.0) + direction
    } else {
        Vec3::new(4.0, 0.0, 2.0)
    };
    commands.spawn((
        MoonlyLight {
            light_type: MoonlyLightType::Direct,
            flags: MoonlyLightFlags::SHADOW,
            color: Vec3::new(2.0, 1.8, 1.5),
            direction,
            ..MoonlyLight::default()
        },
        Transform::from_xyz(0.0, 10.0, 0.0).looking_at(target, Vec3::Y),
    ));
}

pub(crate) const fn glow_component() -> MoonlyGlow {
    MoonlyGlow {
        color: Vec3::new(0.8, 0.25, 0.05),
        lumens: 2500.0,
        samples: 4,
        range: 8.0,
    }
}

pub(crate) fn spawn_transparent(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    if !std::env::args()
        .any(|arg| arg == "--transparent" || arg == "--transmissive" || arg == "--fog-transparent")
    {
        return;
    }
    let transmissive = std::env::args().any(|arg| arg == "--transmissive");
    let material = if transmissive {
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.85, 0.95),
            diffuse_transmission: 0.5,
            specular_transmission: 0.9,
            perceptual_roughness: 0.05,
            thickness: 0.2,
            ior: 1.45,
            cull_mode: None,
            ..StandardMaterial::default()
        })
    } else {
        materials.add(StandardMaterial {
            base_color: Color::srgba(0.1, 0.6, 0.9, 0.35),
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            unlit: true,
            ..StandardMaterial::default()
        })
    };
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.4, 2.4, 0.15))),
        MeshMaterial3d(material),
        Transform::from_xyz(1.5, 2.0, 4.0),
    ));
}

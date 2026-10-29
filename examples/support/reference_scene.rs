use bevy::{
    camera::visibility::RenderLayers,
    pbr::DistanceFog,
    prelude::*,
    render::{
        camera::CameraRenderGraph,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use moonly_core::{
    HemisphereTracker, MoonlyLight, MoonlyLightFlags, MoonlyLightType, MoonlyProjector,
};
use moonly_cull::MoonlyHomOccluder;
use moonly_gi::MoonlyGiCaster;
use moonly_render::{
    MoonlyCamera, MoonlyCameraRole, MoonlyDebugView, MoonlyLinearOutput, MoonlyRender,
    MoonlySurface,
};

use super::numeric_arg;

#[derive(Component)]
pub(crate) struct ReferenceCaster;

pub(crate) fn spawn_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let ground = materials.add(StandardMaterial {
        base_color: Color::srgb(0.24, 0.28, 0.25),
        perceptual_roughness: 0.85,
        ..Default::default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(6.0)))),
        MeshMaterial3d(ground),
        MoonlyGiCaster::default(),
        MoonlyHomOccluder::default(),
    ));

    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.31, 0.16),
        perceptual_roughness: 0.65,
        ..Default::default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(2.0, 2.0, 2.0))),
        MeshMaterial3d(stone),
        Transform::from_xyz(0.0, 1.0, 0.0),
        MoonlyGiCaster::default(),
        MoonlyHomOccluder::default(),
        MoonlySurface {
            material: 0.0,
            ..MoonlySurface::default()
        },
        HemisphereTracker::default(),
    ));

    let metal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.35, 0.48),
        metallic: 0.7,
        perceptual_roughness: 0.3,
        ..Default::default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.8))),
        MeshMaterial3d(metal),
        Transform::from_xyz(2.5, 0.8, -1.0),
        MoonlyGiCaster::default(),
        MoonlyHomOccluder::default(),
        MoonlySurface {
            material: 3.0,
            ..MoonlySurface::default()
        },
        HemisphereTracker::default(),
    ));

    let emissive = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::new(0.8, 0.25, 0.05, 1.0),
        ..Default::default()
    });
    let mut emissive_entity = commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
        MeshMaterial3d(emissive),
        Transform::from_xyz(-2.0, 0.25, 0.5),
        if std::env::args().any(|arg| arg == "--overlay") {
            RenderLayers::layer(1)
        } else {
            RenderLayers::default()
        },
    ));
    if std::env::args().any(|arg| arg == "--glow") {
        emissive_entity.insert(super::native_reference::glow_component());
    }
    spawn_reference_caster(&mut commands, &mut meshes, &mut materials, &mut images);
    super::native_reference::spawn_transparent(&mut commands, &mut meshes, &mut materials);
    spawn_reference_light(&mut commands, &mut images);
    let mut camera = commands.spawn((
        MoonlyCamera::default(),
        debug_view_from_args(),
        CameraRenderGraph::new(MoonlyRender),
        Transform::from_xyz(5.5, 4.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
    if std::env::args().any(|arg| arg == "--linear") {
        camera.insert(MoonlyLinearOutput);
    }
    if std::env::args().any(|arg| arg == "--fog" || arg == "--fog-transparent") {
        camera.insert(DistanceFog::default());
    }
    spawn_overlay_camera(&mut commands);
}

fn spawn_reference_caster(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) {
    let blocker = materials.add(blocker_material(images));
    let mut caster = commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.4, 2.4, 0.3))),
        MeshMaterial3d(blocker),
        Transform::from_xyz(-0.9, 1.2, 1.35),
        ReferenceCaster,
    ));
    if std::env::args().any(|arg| arg == "--translucent-caster") {
        caster.insert(MoonlySurface {
            transmittance: Vec4::new(0.9, 0.2, 0.05, 0.8),
            ..MoonlySurface::default()
        });
    }
}

fn blocker_material(images: &mut Assets<Image>) -> StandardMaterial {
    let mut material = StandardMaterial {
        base_color: Color::srgb(0.18, 0.20, 0.22),
        perceptual_roughness: 0.9,
        ..Default::default()
    };
    if !std::env::args().any(|arg| arg == "--alpha-caster") {
        return material;
    }
    material.alpha_mode = AlphaMode::Mask(0.5);
    material.base_color_texture = Some(images.add(checker_image()));
    material.cull_mode = None;
    material
}

fn checker_image() -> Image {
    let mut data = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let alpha = if (x / 8 + y / 8) % 2 == 0 { 255 } else { 0 };
            data.extend_from_slice(&[alpha, alpha, alpha, alpha]);
        }
    }
    Image::new(
        Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    )
}

pub(crate) fn move_caster(
    mut casters: Query<&mut Transform, With<ReferenceCaster>>,
    mut frame: Local<u32>,
) {
    if !std::env::args().any(|arg| arg == "--moving-caster") {
        return;
    }
    *frame += 1;
    for mut transform in &mut casters {
        transform.translation.x = -0.9 + *frame as f32 * 0.025;
    }
}

pub(crate) fn move_cascade_camera(
    mut cameras: Query<&mut Transform, With<MoonlyCamera>>,
    mut frame: Local<u32>,
) {
    if !std::env::args().any(|arg| arg == "--cascade-motion") {
        return;
    }
    *frame += 1;
    for mut transform in &mut cameras {
        transform.translation.x = 5.5 + *frame as f32 * 0.02;
        transform.look_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y);
    }
}

fn spawn_reference_light(commands: &mut Commands, images: &mut Assets<Image>) {
    if std::env::args().any(|arg| arg == "--glow") {
        return;
    }
    if std::env::args().any(|arg| arg == "--native-light") {
        super::native_reference::spawn_native_spot(commands);
        return;
    }
    let mut light = MoonlyLight::spot(
        Vec3::new(-2.0, 5.0, 3.0),
        Vec3::new(0.3, -1.0, -0.4),
        Vec3::new(12.8, 9.6, 6.0),
        18.0,
        1.4,
    );
    if std::env::args().any(|arg| arg == "--no-shadow") {
        light.flags = MoonlyLightFlags::default();
    }
    if std::env::args().any(|arg| arg == "--no-volumetric") {
        light.volumetric.intensity = 0.0;
    }
    if std::env::args().any(|arg| arg == "--no-gi") {
        light.flags = MoonlyLightFlags(light.flags.0 & !MoonlyLightFlags::GI_ENABLED.0);
    }
    if std::env::args().any(|arg| arg == "--point") {
        light.light_type = MoonlyLightType::Point;
        light.flags = MoonlyLightFlags::default();
    }
    if std::env::args().any(|arg| arg == "--point-shadow") {
        light.light_type = MoonlyLightType::Point;
        light.flags = MoonlyLightFlags::SHADOW;
    }
    let projector = std::env::args()
        .any(|arg| arg == "--projector")
        .then(|| MoonlyProjector {
            texture: images.add(checker_image()),
        });
    let mut entity = commands.spawn((
        light,
        Transform::from_xyz(-2.0, 5.0, 3.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
    if let Some(projector) = projector {
        entity.insert(projector);
    }
    super::native_reference::spawn_sun(commands);
    spawn_additional_spots(commands);
}

fn spawn_additional_spots(commands: &mut Commands) {
    for index in 1..numeric_arg("--spots", 1) {
        let x = (index % 4) as f32 * 2.0 - 3.0;
        let z = (index / 4) as f32 * 2.0 - 2.0;
        let position = Vec3::new(x, 4.0, z);
        let mut light =
            MoonlyLight::spot(position, Vec3::NEG_Y, Vec3::new(1.2, 0.7, 0.4), 8.0, 1.2);
        light.flags = if index < numeric_arg("--shadowed-spots", 1) {
            MoonlyLightFlags::SHADOW
        } else {
            MoonlyLightFlags::default()
        };
        commands.spawn((
            light,
            Transform::from_translation(position).looking_at(Vec3::new(x, 0.0, z), Vec3::Y),
        ));
    }
}

pub(crate) fn debug_view_from_args() -> MoonlyDebugView {
    match numeric_arg("--debug", 0) {
        1 => MoonlyDebugView::Position,
        2 => MoonlyDebugView::Normal,
        3 => MoonlyDebugView::Albedo,
        4 => MoonlyDebugView::Depth,
        5 => MoonlyDebugView::Accumulator,
        6 => MoonlyDebugView::ShadowAtlas,
        7 => MoonlyDebugView::AmbientOcclusion,
        8 => MoonlyDebugView::Volumetrics,
        9 => MoonlyDebugView::TransmittanceAtlas,
        _ => MoonlyDebugView::Final,
    }
}

fn spawn_overlay_camera(commands: &mut Commands) {
    if !std::env::args().any(|arg| arg == "--overlay") {
        return;
    }
    commands.spawn((
        MoonlyCamera {
            role: MoonlyCameraRole::Overlay,
        },
        Camera {
            order: 1,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..Camera::default()
        },
        RenderLayers::layer(1),
        CameraRenderGraph::new(MoonlyRender),
        Transform::from_xyz(5.5, 4.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
}

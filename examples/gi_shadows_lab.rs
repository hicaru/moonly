//! Interactive and automated GI radiosity, wet rain shadow, and optics laboratory example.
#[allow(dead_code)]
mod support;
use bevy::{app::AppExit, prelude::*};
use moonly::prelude::*;
use support::{
    CaptureOptions, CaptureState, capture_system, deterministic_time,
    scene::{ExampleCamera, ExampleLight, ExampleOrb, MotionEnabled, animate_scene, spawn_camera},
    window_plugin,
};
#[derive(Resource, Debug, Clone, Copy, Default)]
struct LabOptions {
    optics_index: u32,
    red_wall: bool,
    no_gi: bool,
}
impl LabOptions {
    fn from_env_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let index = args.iter().position(|arg| arg == "--optics");
        Self {
            optics_index: index
                .and_then(|i| args.get(i + 1))
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
            red_wall: args.iter().any(|arg| arg == "--red-wall"),
            no_gi: args.iter().any(|arg| arg == "--no-gi"),
        }
    }
}
fn main() -> AppExit {
    let options = LabOptions::from_env_args();
    let mut config = MoonlyConfig {
        rain_wetness: 0.65,
        ..MoonlyConfig::default()
    };
    if options.red_wall {
        config.tier = MoonlyPipelineTier::MoonlyEnhanced;
    }
    App::new()
        .add_plugins(DefaultPlugins.set(window_plugin(
            "Moonly — GI Radiosity, Rain Wetness & Optics Lab",
        )))
        .add_plugins(MoonlyPlugin { config })
        .insert_resource(ClearColor(Color::srgb(0.14, 0.18, 0.25)))
        .insert_resource(CaptureOptions::from_env_args(
            "target/visual-audit/gi-lab.png",
        ))
        .insert_resource(options)
        .insert_resource(CaptureState::default())
        .insert_resource(MotionEnabled::from_env_args())
        .insert_resource(deterministic_time())
        .add_systems(Startup, setup_gi_lab_scene)
        .add_systems(
            Update,
            (animate_scene, report_red_wall_gi, capture_system).chain(),
        )
        .run()
}
const LAB_BLOCKERS: [Vec4; 4] = [
    Vec4::new(-1.6, 0.75, 0.2, 0.75),
    Vec4::new(0.0, 0.75, 0.2, 0.75),
    Vec4::new(1.6, 0.75, 0.2, 0.75),
    Vec4::ZERO,
];
fn setup_gi_lab_scene(
    mut commands: Commands,
    options: Res<LabOptions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
    mut post_cfg: ResMut<Hdr10PostConfig>,
) {
    post_cfg.operator = TonemapOperator::AgX;
    post_cfg.exposure = if options.red_wall { 0.55 } else { 1.25 };
    if options.red_wall {
        commands.spawn((
            Camera3d::default(),
            ExampleCamera,
            Transform::from_xyz(-1.5, 2.7, 9.2).looking_at(Vec3::new(-0.55, 1.3, 0.2), Vec3::Y),
        ));
        spawn_red_wall_scene(&mut commands, &mut meshes, &mut materials, options.no_gi);
        return;
    }
    spawn_camera(&mut commands);
    spawn_cornell_bunker(&mut commands, &mut meshes, &mut materials);
    spawn_bunker_spheres_and_lights(
        &mut commands,
        &mut meshes,
        &mut materials,
        options.optics_index,
    );
}
struct LabBox {
    center: Vec3,
    size: Vec3,
    albedo: Vec3,
    gi_albedo: Vec3,
    roughness: f32,
    blockers: bool,
}
fn spawn_lab_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
    spec: LabBox,
) {
    let mut material = MoonlyLitMaterial::from_albedo_roughness(spec.albedo, spec.roughness);
    if spec.blockers {
        material = material.with_blockers(LAB_BLOCKERS);
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::from_size(spec.size))),
        MeshMaterial3d(materials.add(material)),
        Transform::from_translation(spec.center),
        GiReflectorBox::from_center_extents(spec.center, spec.size * 0.5, spec.gi_albedo),
    ));
}
fn spawn_cornell_bunker(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
) {
    for spec in [
        LabBox {
            center: Vec3::new(0.0, -0.15, 0.0),
            size: Vec3::new(14.0, 0.3, 14.0),
            albedo: Vec3::new(0.46, 0.45, 0.43),
            gi_albedo: Vec3::new(0.55, 0.54, 0.52),
            roughness: 0.16,
            blockers: true,
        },
        LabBox {
            center: Vec3::new(-3.4, 2.0, -0.5),
            size: Vec3::new(0.4, 4.0, 6.0),
            albedo: Vec3::new(0.86, 0.36, 0.18),
            gi_albedo: Vec3::new(0.90, 0.38, 0.18),
            roughness: 0.55,
            blockers: true,
        },
        LabBox {
            center: Vec3::new(3.4, 2.0, -0.5),
            size: Vec3::new(0.4, 4.0, 6.0),
            albedo: Vec3::new(0.18, 0.68, 0.78),
            gi_albedo: Vec3::new(0.18, 0.72, 0.85),
            roughness: 0.55,
            blockers: true,
        },
    ] {
        spawn_lab_box(commands, meshes, materials, spec);
    }
}
fn spawn_bunker_spheres_and_lights(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
    optics_index: u32,
) {
    let sphere_mesh = meshes.add(Sphere::new(0.75).mesh().uv(32, 18));
    for (idx, x_pos) in [-1.6_f32, 0.0, 1.6].into_iter().enumerate() {
        let roughness = 0.12 + (idx as f32) * 0.32;
        let mat = materials.add(
            MoonlyLitMaterial::from_albedo_roughness(Vec3::new(0.82, 0.80, 0.76), roughness)
                .with_blockers(LAB_BLOCKERS),
        );
        commands.spawn((
            Mesh3d(sphere_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_xyz(x_pos, 0.75, 0.2),
            HemisphereTracker::default(),
            ExampleOrb(idx as u32),
        ));
    }
    commands.spawn((
        MoonlyLight::spot(
            Vec3::new(-1.2, 4.2, 1.5),
            Vec3::new(0.25, -0.92, -0.28).normalize(),
            Vec3::new(3.4, 2.5, 1.6),
            18.0,
            1.35,
        ),
        Transform::from_xyz(-1.2, 4.2, 1.5),
        GlobalTransform::default(),
        ExampleLight,
    ));
    commands.spawn((
        MoonlyLight {
            flags: MoonlyLightFlags::GI_ENABLED,
            color: Vec3::new(0.5, 0.8, 1.4),
            position: Vec3::new(1.8, 2.4, 1.0),
            range: 10.0,
            ..MoonlyLight::default()
        },
        Transform::from_xyz(1.8, 2.4, 1.0),
        GlobalTransform::default(),
    ));
    commands.spawn((
        MoonlyLight {
            light_type: MoonlyLightType::Direct,
            flags: MoonlyLightFlags::GI_ENABLED,
            color: Vec3::new(0.18, 0.16, 0.13),
            direction: Vec3::new(-0.4, -0.8, -0.3).normalize(),
            range: 60.0,
            ..MoonlyLight::default()
        },
        Transform::default().looking_at(Vec3::new(-0.4, -0.8, -0.3), Vec3::Y),
    ));
    let mode = match optics_index {
        1 => OpticsMode::NightVisionGreen,
        2 => OpticsMode::NightVisionWhite,
        3 => OpticsMode::ThermalHeatVision,
        4 => OpticsMode::ParallaxScope,
        _ => OpticsMode::Standard,
    };
    commands.insert_resource(mode);
}
fn spawn_red_wall_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
    no_gi: bool,
) {
    for spec in [
        LabBox {
            center: Vec3::new(-2.4, 1.8, 0.0),
            size: Vec3::new(0.24, 3.6, 5.4),
            albedo: Vec3::new(0.86, 0.008, 0.012),
            gi_albedo: Vec3::new(0.86, 0.008, 0.012),
            roughness: 0.78,
            blockers: false,
        },
        LabBox {
            center: Vec3::new(1.1, 1.8, 0.0),
            size: Vec3::new(0.24, 3.6, 5.4),
            albedo: Vec3::splat(0.62),
            gi_albedo: Vec3::splat(0.62),
            roughness: 0.62,
            blockers: false,
        },
        LabBox {
            center: Vec3::new(-0.65, -0.12, 0.0),
            size: Vec3::new(6.8, 0.24, 6.4),
            albedo: Vec3::splat(0.46),
            gi_albedo: Vec3::splat(0.46),
            roughness: 0.5,
            blockers: false,
        },
    ] {
        spawn_lab_box(commands, meshes, materials, spec);
    }
    let receiver = materials.add(MoonlyLitMaterial::from_albedo_roughness(
        Vec3::splat(0.78),
        0.32,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.8).mesh().uv(48, 24))),
        MeshMaterial3d(receiver),
        Transform::from_xyz(-1.3, 0.82, 0.8),
        HemisphereTracker::default(),
    ));
    let position = Vec3::new(-1.65, 1.55, 0.6);
    let mut light = MoonlyLight::spot(position, Vec3::NEG_X, Vec3::splat(12.0), 12.0, 1.1);
    if no_gi {
        light.flags = MoonlyLightFlags(light.flags.0 & !MoonlyLightFlags::GI_ENABLED.0);
    }
    commands.spawn((
        light,
        Transform::from_translation(position).looking_to(Vec3::NEG_X, Vec3::Y),
        GlobalTransform::default(),
    ));
}
fn report_red_wall_gi(
    options: Res<LabOptions>,
    virtual_lights: Res<moonly_core::VirtualGiLightPackage>,
    time: Res<Time>,
    mut cameras: Query<&mut Transform, With<ExampleCamera>>,
    mut frame: Local<u32>,
) {
    if options.red_wall {
        let t = time.elapsed_secs();
        for mut transform in &mut cameras {
            *transform = Transform::from_xyz(-1.5 + t.sin() * 0.18, 2.7, 9.2)
                .looking_at(Vec3::new(-0.55, 1.3, 0.2), Vec3::Y);
        }
    }
    *frame += 1;
    if *frame != 45 || !options.red_wall {
        return;
    }
    let lights = &virtual_lights.lights;
    let red_energy = lights.iter().map(|light| light.color.x).sum::<f32>();
    let max_range = lights
        .iter()
        .fold(0.0_f32, |range, light| range.max(light.range));
    info!(
        "Red wall GI: {} virtual lights, red energy {:.6}, max range {:.6}",
        lights.len(),
        red_energy,
        max_range
    );
}

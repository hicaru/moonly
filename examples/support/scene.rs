use bevy::{ecs::system::SystemParam, prelude::*};
use moonly::prelude::*;

#[derive(Component)]
pub(crate) struct ExampleCamera;

#[derive(Component)]
pub(crate) struct ExampleLight;

#[derive(Component)]
pub(crate) struct ExampleOrb(pub(crate) u32);

#[derive(Resource)]
pub(crate) struct MotionEnabled(bool);

impl MotionEnabled {
    pub(crate) fn from_env_args() -> Self {
        Self(std::env::args().any(|arg| arg == "--motion"))
    }
}

type AnimationResources<'w> = (
    Res<'w, Time>,
    Res<'w, MotionEnabled>,
    ResMut<'w, Assets<MoonlyLitMaterial>>,
);

type CameraQuery<'w, 's> = Query<'w, 's, &'static mut Transform, With<ExampleCamera>>;
type LightQuery<'w, 's> = Query<'w, 's, &'static mut Transform, With<ExampleLight>>;
type OrbQuery<'w, 's> = Query<'w, 's, (&'static mut Transform, &'static ExampleOrb)>;

#[derive(SystemParam)]
pub(crate) struct AnimationQueries<'w, 's> {
    queries: ParamSet<'w, 's, (CameraQuery<'w, 's>, LightQuery<'w, 's>, OrbQuery<'w, 's>)>,
}

pub(crate) const SCENE_BLOCKERS: [Vec4; 4] = [
    Vec4::new(-1.7, 0.8, 0.0, 0.8),
    Vec4::new(0.0, 0.8, 0.0, 0.8),
    Vec4::new(1.7, 0.8, 0.0, 0.8),
    Vec4::ZERO,
];

struct BoxSpec {
    center: Vec3,
    size: Vec3,
    albedo: Vec3,
}

pub(crate) fn spawn_test_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
) {
    spawn_box(
        commands,
        meshes,
        materials,
        BoxSpec {
            center: Vec3::new(0.0, -0.15, 0.0),
            size: Vec3::new(14.0, 0.3, 14.0),
            albedo: Vec3::new(0.48, 0.46, 0.43),
        },
    );
    for (x, color) in [
        (-3.4, Vec3::new(0.85, 0.28, 0.15)),
        (3.4, Vec3::new(0.15, 0.62, 0.85)),
    ] {
        spawn_box(
            commands,
            meshes,
            materials,
            BoxSpec {
                center: Vec3::new(x, 2.0, -0.5),
                size: Vec3::new(0.35, 4.0, 6.0),
                albedo: color,
            },
        );
    }
    let sphere = meshes.add(Sphere::new(0.8).mesh().uv(32, 18));
    for (index, x) in [-1.7_f32, 0.0, 1.7].into_iter().enumerate() {
        let material = MoonlyLitMaterial::from_albedo_roughness(
            Vec3::new(0.82, 0.80, 0.76),
            0.12 + index as f32 * 0.3,
        )
        .with_blockers(SCENE_BLOCKERS);
        commands.spawn((
            Mesh3d(sphere.clone()),
            MeshMaterial3d(materials.add(material)),
            Transform::from_xyz(x, 0.8, 0.0),
            HemisphereTracker::default(),
            ExampleOrb(index as u32),
        ));
    }
    commands.spawn((
        MoonlyLight::spot(
            Vec3::new(-1.0, 4.5, 1.8),
            Vec3::new(0.2, -1.0, -0.35),
            Vec3::new(3.2, 2.4, 1.5),
            18.0,
            1.4,
        ),
        Transform::from_xyz(-1.0, 4.5, 1.8),
        GlobalTransform::default(),
        ExampleLight,
    ));
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<MoonlyLitMaterial>,
    spec: BoxSpec,
) {
    let material =
        MoonlyLitMaterial::from_albedo_roughness(spec.albedo, 0.42).with_blockers(SCENE_BLOCKERS);
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::from_size(spec.size))),
        MeshMaterial3d(materials.add(material)),
        Transform::from_translation(spec.center),
        GiReflectorBox::from_center_extents(spec.center, spec.size * 0.5, spec.albedo),
    ));
}

pub(crate) fn spawn_camera(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        ExampleCamera,
        Transform::from_xyz(0.0, 2.6, 7.2).looking_at(Vec3::new(0.0, 1.1, 0.0), Vec3::Y),
    ));
}

pub(crate) fn animate_scene(resources: AnimationResources, mut queries: AnimationQueries) {
    let (time, motion, mut materials) = resources;
    if !motion.0 {
        return;
    }
    let seconds = time.elapsed_secs();
    for mut transform in &mut queries.queries.p0() {
        *transform = Transform::from_xyz(
            (seconds * 0.7).sin() * 1.2,
            2.6 + (seconds * 0.5).cos() * 0.2,
            7.2,
        )
        .looking_at(Vec3::new(0.0, 1.1, 0.0), Vec3::Y);
    }
    for mut transform in &mut queries.queries.p1() {
        *transform = Transform::from_xyz(-1.0 + (seconds * 0.9).sin(), 4.5, 1.8)
            .looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y);
    }
    for (mut transform, marker) in &mut queries.queries.p2() {
        transform.translation = orb_position(marker.0, seconds);
    }
    let [b0, b1, b2, _] = animated_blockers(seconds);
    for (_, material) in materials.iter_mut() {
        material.uniform.blocker_0 = b0;
        material.uniform.blocker_1 = b1;
        material.uniform.blocker_2 = b2;
    }
}

fn orb_position(index: u32, seconds: f32) -> Vec3 {
    let phase = seconds * 1.4 + index as f32 * 0.9;
    Vec3::new(
        (index as f32 - 1.0) * 1.7,
        0.8 + phase.sin() * 0.25,
        phase.cos() * 0.3,
    )
}

fn animated_blockers(seconds: f32) -> [Vec4; 4] {
    [
        orb_position(0, seconds).extend(0.8),
        orb_position(1, seconds).extend(0.8),
        orb_position(2, seconds).extend(0.8),
        Vec4::ZERO,
    ]
}

use bevy::asset::{AssetApp, RenderAssetUsages};
use moonly_core::{MoonlyConfig, MoonlyLight, MoonlyLightFlags};

use super::*;
use crate::{GiLightCache, GiLightField, MoonlyGiPlugin};

fn indexed_triangle() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

fn geometry_with_mesh(mesh: &Mesh, transform: &GlobalTransform) -> StaticGiGeometry {
    let mut triangles = Vec::new();
    append_mesh_triangles(mesh, transform, true, &mut triangles);
    let mut geometry = StaticGiGeometry::default();
    geometry.rebuild(&mut triangles);
    geometry
}

#[test]
fn mesh_indices_and_world_transform_create_geometric_triangles() {
    let transform = GlobalTransform::from(
        Transform::from_translation(Vec3::new(2.0, 3.0, 4.0)).with_scale(Vec3::splat(2.0)),
    );
    let geometry = geometry_with_mesh(&indexed_triangle(), &transform);
    assert_eq!(geometry.bvh().triangle_count(), 1);
    let ray = GiRayQuery::new(Vec3::new(2.5, 7.0, 4.5), Vec3::NEG_Y, 0.0, 10.0);
    let hit = geometry.nearest_hit(ray);
    assert_eq!(hit.map(|hit| hit.normal), Some(Vec3::Y));
    assert!((hit.map_or(0.0, |hit| hit.distance) - 4.0).abs() < 1.0e-5);
}

#[test]
fn geometry_revision_changes_only_for_relevant_scene_changes() {
    let mut app = App::new();
    app.add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_resource::<MoonlyConfig>()
        .add_plugins(MoonlyGiPlugin);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(indexed_triangle());
    let caster = app
        .world_mut()
        .spawn((
            Mesh3d(handle.clone()),
            GlobalTransform::default(),
            MoonlyGiCaster::default(),
        ))
        .id();
    app.update();
    let first = app.world().resource::<StaticGiGeometry>().revision();
    app.update();
    assert_eq!(app.world().resource::<StaticGiGeometry>().revision(), first);

    if let Some(mut mesh) = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .get_mut(&handle)
    {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 0.0]],
        );
    }
    app.update();
    let edited = app.world().resource::<StaticGiGeometry>().revision();
    assert!(edited > first);
    app.world_mut()
        .entity_mut(caster)
        .insert(GlobalTransform::from_xyz(0.0, 1.0, 0.0));
    app.update();
    let moved = app.world().resource::<StaticGiGeometry>().revision();
    assert!(moved > edited);
    app.world_mut().entity_mut(caster).despawn();
    app.update();
    assert!(app.world().resource::<StaticGiGeometry>().revision() > moved);
    assert_eq!(
        app.world()
            .resource::<StaticGiGeometry>()
            .bvh()
            .triangle_count(),
        0
    );
}

#[test]
fn skinned_meshes_are_excluded_and_plugin_runs_without_assets() {
    let mut app = App::new();
    app.add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_resource::<MoonlyConfig>()
        .add_plugins(MoonlyGiPlugin);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(indexed_triangle());
    app.world_mut().spawn((
        Mesh3d(handle),
        GlobalTransform::default(),
        MoonlyGiCaster::default(),
        SkinnedMesh::default(),
    ));
    app.update();
    assert_eq!(
        app.world()
            .resource::<StaticGiGeometry>()
            .bvh()
            .triangle_count(),
        0
    );

    let mut headless = App::new();
    headless
        .init_resource::<MoonlyConfig>()
        .add_plugins(MoonlyGiPlugin);
    headless.update();
    assert_eq!(
        headless
            .world()
            .resource::<GiLightField>()
            .virtual_lights
            .len(),
        0
    );
}

#[test]
fn unchanged_plugin_frames_cast_no_rays_and_light_removal_clears_cache() {
    let mut app = App::new();
    app.add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_resource::<MoonlyConfig>()
        .add_plugins(MoonlyGiPlugin);
    let handle = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(indexed_triangle());
    app.world_mut().spawn((
        Mesh3d(handle),
        GlobalTransform::default(),
        MoonlyGiCaster::default(),
    ));
    let light = app
        .world_mut()
        .spawn(MoonlyLight {
            position: Vec3::new(0.0, 2.0, 0.0),
            flags: MoonlyLightFlags::GI_ENABLED,
            ..MoonlyLight::default()
        })
        .id();
    app.update();
    let first_rays = app.world().resource::<StaticGiGeometry>().rays_traced();
    assert!(first_rays > 0);
    app.update();
    assert_eq!(
        app.world().resource::<StaticGiGeometry>().rays_traced(),
        first_rays
    );
    assert_eq!(app.world().resource::<GiLightCache>().light_count(), 1);

    app.world_mut().entity_mut(light).despawn();
    app.update();
    assert_eq!(app.world().resource::<GiLightCache>().light_count(), 0);
    assert_eq!(
        app.world().resource::<GiLightField>().virtual_lights.len(),
        0
    );
}

use bevy::asset::AssetApp;
use moonly_core::{LightPackage, MoonlyCorePlugin};

use super::*;
use crate::MoonlyGiPlugin;

fn glow_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        MoonlyCorePlugin,
        MoonlyGiPlugin,
    ));
    app.init_asset::<Mesh>();
    app
}

fn spawn_emitter(app: &mut App, glow: MoonlyGlow) -> Entity {
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Plane3d::new(Vec3::Y, Vec2::splat(1.0)));
    app.world_mut()
        .spawn((
            Mesh3d(mesh),
            Transform::default(),
            InheritedVisibility::VISIBLE,
            glow,
        ))
        .id()
}

#[test]
fn mesh_emitter_preserves_total_lumens_and_reuses_warm_cache() {
    let mut app = glow_test_app();
    let lumens = FULL_SPHERE_STERADIANS * 4.0;
    spawn_emitter(
        &mut app,
        MoonlyGlow {
            lumens,
            samples: 4,
            ..MoonlyGlow::default()
        },
    );
    app.update();
    app.update();
    let cache = app.world().resource::<MoonlyGlowCache>();
    assert_eq!(cache.light_count(), 4);
    let revision = cache.revision();
    let total = app
        .world()
        .resource::<LightPackage>()
        .v_point
        .iter()
        .fold(Vec3::ZERO, |sum, light| sum + light.color)
        * FULL_SPHERE_STERADIANS;
    assert!((total.x - lumens).abs() < 1.0e-3);
    assert!((total.y - lumens).abs() < 1.0e-3);
    assert!((total.z - lumens).abs() < 1.0e-3);

    app.update();
    assert_eq!(
        app.world().resource::<MoonlyGlowCache>().revision(),
        revision
    );
}

#[test]
fn transform_and_removal_update_only_the_emitter_cache() {
    let mut app = glow_test_app();
    let entity = spawn_emitter(&mut app, MoonlyGlow::default());
    app.update();
    app.update();
    let revision = app.world().resource::<MoonlyGlowCache>().revision();
    if let Some(mut transform) = app.world_mut().get_mut::<GlobalTransform>(entity) {
        *transform = GlobalTransform::from_translation(Vec3::new(3.0, 0.0, 0.0));
    }
    app.update();
    assert!(app.world().resource::<MoonlyGlowCache>().revision() > revision);
    assert_eq!(app.world().resource::<MoonlyGlowCache>().light_count(), 4);

    app.world_mut().entity_mut(entity).remove::<MoonlyGlow>();
    app.update();
    assert_eq!(app.world().resource::<MoonlyGlowCache>().light_count(), 0);
}

#[test]
fn hidden_and_skinned_emitters_do_not_spawn_lights() {
    let mut app = glow_test_app();
    spawn_emitter(&mut app, MoonlyGlow::default());
    let hidden = spawn_emitter(&mut app, MoonlyGlow::default());
    app.world_mut()
        .entity_mut(hidden)
        .insert(InheritedVisibility::HIDDEN);
    let skinned = spawn_emitter(&mut app, MoonlyGlow::default());
    app.world_mut().entity_mut(skinned).insert(SkinnedMesh {
        inverse_bindposes: Handle::default(),
        joints: Vec::new(),
    });
    app.update();
    assert_eq!(app.world().resource::<MoonlyGlowCache>().light_count(), 4);
}

#[test]
fn emitter_layers_are_copied_to_every_sample() {
    let mut app = glow_test_app();
    let entity = spawn_emitter(&mut app, MoonlyGlow::default());
    app.world_mut()
        .entity_mut(entity)
        .insert(RenderLayers::layer(2));
    app.update();
    assert!(
        app.world()
            .resource::<LightPackage>()
            .v_point
            .iter()
            .all(|light| light.visible_in_layers(&RenderLayers::layer(2))
                && !light.visible_in_layers(&RenderLayers::default()))
    );
}

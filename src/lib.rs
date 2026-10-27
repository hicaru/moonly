//! Moonly — Bevy lighting, dynamic shadows, GI radiosity, and HDR10 plugin ported from `moonly-monolith`.

pub mod prelude;

pub use bevy;
pub use moonly_core as core;
pub use moonly_cull as cull;
pub use moonly_gi as gi;
pub use moonly_post as post;
pub use moonly_render as render;
pub use moonly_shadows as shadows;
pub use moonly_volumetrics as volumetrics;

use bevy::prelude::*;
use moonly_core::{MoonlyConfig, MoonlyCorePlugin, MoonlyNativeForward};
use moonly_cull::MoonlyCullPlugin;
use moonly_gi::MoonlyGiPlugin;
use moonly_render::MoonlyRenderPlugin;
use moonly_shadows::MoonlyShadowsPlugin;

/// Master Bevy plugin that installs all Moonly lighting, culling, shadow, GI, volumetric, and post plugins.
#[derive(Debug, Default, Clone)]
pub struct MoonlyPlugin {
    /// Initial master configuration resource.
    pub config: MoonlyConfig,
}

impl MoonlyPlugin {
    /// Construct the plugin with the enhanced Moonly configuration preset.
    #[must_use]
    pub fn enhanced() -> Self {
        Self {
            config: MoonlyConfig::enhanced(),
        }
    }
}

impl Plugin for MoonlyPlugin {
    fn build(&self, app: &mut App) {
        let native_forward = app
            .world()
            .get_resource::<MoonlyNativeForward>()
            .is_some_and(|settings| settings.0);
        if native_forward {
            app.add_plugins(MoonlyRenderPlugin);
        } else {
            app.add_plugins((
                MoonlyCorePlugin,
                MoonlyRenderPlugin,
                MoonlyCullPlugin,
                MoonlyShadowsPlugin,
                MoonlyGiPlugin,
            ));
        }
        app.insert_resource(self.config.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::prelude::*;

    #[test]
    fn full_moonly_plugin_ticks_all_subsystems_in_headless_app() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, MoonlyPlugin::enhanced()));
        app.world_mut().spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 2.0, 8.0),
            GlobalTransform::from(Transform::from_xyz(0.0, 2.0, 8.0)),
        ));

        let light_transform = Transform::from_xyz(0.0, 4.0, 0.0).looking_at(Vec3::ZERO, Vec3::Z);
        app.world_mut().spawn((
            MoonlyLight::spot(
                Vec3::new(0.0, 4.0, 0.0),
                Vec3::NEG_Y,
                Vec3::new(3.0, 2.5, 2.0),
                16.0,
                1.2,
            ),
            light_transform,
            GlobalTransform::from(light_transform),
        ));
        app.world_mut().spawn(GiReflectorBox::from_center_extents(
            Vec3::new(0.0, -0.5, 0.0),
            Vec3::new(8.0, 0.5, 8.0),
            Vec3::new(0.75, 0.55, 0.35),
        ));
        app.world_mut().spawn(HemisphereTracker::default());

        app.update();

        let pkg = app.world().resource::<LightPackage>();
        assert_eq!(pkg.v_shadowed.len(), 1);

        let gi = app.world().resource::<GiLightField>();
        assert!(!gi.virtual_lights.is_empty());

        let batches = app.world().resource::<LocalShadowBatches>();
        assert_eq!(batches.view_count(), 1);
        assert!(
            batches
                .active()
                .first()
                .is_some_and(|batch| batch.allocator.allocated.len() == 1)
        );
    }

    #[test]
    fn native_forward_avoids_unused_subsystems() {
        let mut app = App::new();
        app.insert_resource(MoonlyNativeForward(true));
        app.insert_resource(MoonlyNativePost(true));
        app.add_plugins((MinimalPlugins, MoonlyPlugin::enhanced()));
        app.world_mut().spawn(MoonlyCamera::default());
        app.update();
        assert!(app.world().contains_resource::<MoonlyGpuDiagnostics>());
        assert!(!app.world().contains_resource::<LightPackage>());
        assert!(!app.world().contains_resource::<WeatherEnvironment>());
        assert!(!app.world().contains_resource::<GiLightField>());
        assert!(!app.world().contains_resource::<OccRasterizer>());
        assert!(!app.world().contains_resource::<LocalShadowBatches>());
    }

    #[test]
    fn material_sync_creates_no_phantom_lights_and_clears_removed_lights() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, MoonlyPlugin::enhanced()));
        app.init_resource::<Assets<MoonlyLitMaterial>>();
        app.add_systems(
            PostUpdate,
            moonly_post::sync_moonly_lit_materials
                .in_set(moonly_core::MoonlySystemSet::SyncMaterials),
        );
        let handle = app
            .world_mut()
            .resource_mut::<Assets<MoonlyLitMaterial>>()
            .add(MoonlyLitMaterial::default());
        app.update();
        let material = app
            .world()
            .resource::<Assets<MoonlyLitMaterial>>()
            .get(&handle);
        assert!(material.is_some_and(|mat| mat.uniform.spot_color_vol == Vec4::ZERO));
        assert!(material.is_some_and(|mat| mat.uniform.gi_color_0 == Vec4::ZERO));

        let entity = app
            .world_mut()
            .spawn((
                MoonlyLight::spot(Vec3::Y, Vec3::NEG_Y, Vec3::ONE, 8.0, 1.0),
                Transform::from_translation(Vec3::Y),
                GlobalTransform::default(),
            ))
            .id();
        app.update();
        let lit = app
            .world()
            .resource::<Assets<MoonlyLitMaterial>>()
            .get(&handle);
        assert!(lit.is_some_and(|mat| mat.uniform.spot_color_vol.truncate() == Vec3::ONE));

        app.world_mut().despawn(entity);
        app.update();
        let cleared = app
            .world()
            .resource::<Assets<MoonlyLitMaterial>>()
            .get(&handle);
        assert!(cleared.is_some_and(|mat| mat.uniform.spot_color_vol == Vec4::ZERO));

        app.world_mut().spawn((
            MoonlyLight {
                light_type: MoonlyLightType::Direct,
                color: Vec3::new(2.0, 3.0, 4.0),
                ..MoonlyLight::default()
            },
            Transform::default(),
            GlobalTransform::default(),
        ));
        app.update();
        let direct = app
            .world()
            .resource::<Assets<MoonlyLitMaterial>>()
            .get(&handle);
        assert!(direct.is_some_and(|mat| {
            mat.uniform.sun_dir_shadow_softness.truncate() == Vec3::NEG_Z
                && mat.uniform.sun_color_wetness.truncate() == Vec3::new(2.0, 3.0, 4.0)
        }));
    }

    #[test]
    fn standalone_wesl_shader_libraries_compile_cleanly() {
        let shaders = [
            ("crates/moonly_core/src", "package::core_common"),
            ("crates/moonly_shadows/src", "package::shadows"),
            ("crates/moonly_gi/src", "package::gi"),
            ("crates/moonly_volumetrics/src", "package::volumetrics"),
            ("crates/moonly_post/src", "package::tonemap"),
            ("crates/moonly_post/src", "package::bloom_optics"),
        ];
        for (dir, module_path) in shaders {
            let parsed = module_path.parse();
            assert!(parsed.is_ok());
            if let Ok(p) = parsed {
                let compiled = wesl::Wesl::new(dir).compile(&p);
                assert!(compiled.is_ok());
            }
        }
    }
}

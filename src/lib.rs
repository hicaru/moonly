//! Moonly — Bevy lighting, dynamic shadows, GI radiosity, and HDR10 plugin ported from `xray-monolith`.

pub mod prelude;

pub use bevy;
pub use moonly_core as core;
pub use moonly_cull as cull;
pub use moonly_gi as gi;
pub use moonly_post as post;
pub use moonly_shadows as shadows;
pub use moonly_volumetrics as volumetrics;

use bevy::prelude::*;
use moonly_core::{MoonlyConfig, MoonlyCorePlugin};
use moonly_cull::MoonlyCullPlugin;
use moonly_gi::MoonlyGiPlugin;
use moonly_post::MoonlyPostPlugin;
use moonly_shadows::MoonlyShadowsPlugin;
use moonly_volumetrics::MoonlyVolumetricsPlugin;

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
        app.add_plugins((
            MoonlyCorePlugin,
            MoonlyCullPlugin,
            MoonlyShadowsPlugin,
            MoonlyGiPlugin,
            MoonlyVolumetricsPlugin,
            MoonlyPostPlugin,
        ))
        .insert_resource(self.config.clone());
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
            XRayLight::spot(
                Vec3::new(0.0, 4.0, 0.0),
                Vec3::NEG_Y,
                Vec3::new(3.0, 2.5, 2.0),
                16.0,
                1.2,
            ),
            Transform::from_xyz(0.0, 4.0, 0.0),
            GlobalTransform::default(),
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

        let alloc = app.world().resource::<SmapAllocator>();
        assert_eq!(alloc.allocated.len(), 1);
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

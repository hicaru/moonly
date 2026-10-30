use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

const COMPAT_PLUGIN: &str = r#"
.insert_resource(moonly::prelude::MoonlyNativeForward(
    std::env::var_os("MOONLY_VANILLA").is_some(),
))
.insert_resource(moonly::prelude::MoonlyNativePost(
    std::env::var_os("MOONLY_VANILLA").is_some()
        || std::env::var_os("MOONLY_NATIVE_POST").is_some(),
))
.insert_resource(moonly_example_time_strategy())
.add_plugins(moonly::MoonlyPlugin {
    config: moonly_example_config(),
})
.add_systems(Last, (moonly_example_test_overrides, moonly_example_camera_diagnostics))"#;
const MATERIAL_PLUGIN: &str = ".add_plugins(MoonlyExampleCompatPlugin)";
const RENDER_MODE_HELPER: &str = r#"
fn moonly_example_time_strategy() -> bevy::time::TimeUpdateStrategy {
    if std::env::var_os("MOONLY_DETERMINISTIC_TIME").is_some() {
        bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(
            1.0 / 60.0,
        ))
    } else {
        bevy::time::TimeUpdateStrategy::Automatic
    }
}

fn moonly_example_config() -> moonly::prelude::MoonlyConfig {
    let mut config = if std::env::var_os("MOONLY_MAX_QUALITY").is_some() {
        moonly::prelude::MoonlyConfig::enhanced()
    } else {
        moonly::prelude::MoonlyConfig::default()
    };
    if std::env::var_os("MOONLY_MAX_QUALITY").is_some() {
        config.smap_pool_size = 8192;
        config.gi.num_photons = 64;
        config.volumetrics.quality = 2.0;
        config.volumetrics.slice_count = 256;
        config.rain_wet_surface.smap_resolution = 1024;
    }
    config
}

fn moonly_example_test_overrides(
    mut cameras: Query<(
        &mut bevy::core_pipeline::tonemapping::Tonemapping,
        &mut bevy::camera::Exposure,
    )>,
    mut lights: Query<&mut moonly::prelude::MoonlyLight>,
    environment: Option<ResMut<moonly::prelude::WeatherEnvironment>>,
    config: Option<ResMut<moonly::prelude::MoonlyConfig>>,
) {
    for (mut tonemapping, mut exposure) in &mut cameras {
        if std::env::var_os("MOONLY_LINEAR_TONEMAP").is_some() {
            *tonemapping = bevy::core_pipeline::tonemapping::Tonemapping::Linear;
        }
        if let Ok(ev100) = std::env::var("MOONLY_EV100") {
            exposure.ev100 = ev100.parse().unwrap_or(exposure.ev100);
        }
    }
    for mut light in &mut lights {
        if std::env::var_os("MOONLY_NO_SHADOWS").is_some() {
            light.flags.0 &= !moonly::prelude::MoonlyLightFlags::SHADOW.0;
        }
        if std::env::var_os("MOONLY_NO_GI").is_some() {
            light.flags.0 &= !moonly::prelude::MoonlyLightFlags::GI_ENABLED.0;
        }
    }
    if std::env::var_os("MOONLY_NO_AMBIENT").is_some()
        && let Some(mut environment) = environment
    {
        environment.ambient_color = Vec3::ZERO;
    }
    if std::env::var_os("MOONLY_NO_AO").is_some()
        && let Some(mut config) = config
    {
        config.ao_mode = moonly::prelude::AmbientOcclusionMode::Off;
    }
}

fn moonly_example_camera_diagnostics(
    cameras: Query<(
        &bevy::render::camera::CameraRenderGraph,
        Has<moonly::prelude::MoonlyCamera>,
        Has<bevy::core_pipeline::prepass::DeferredPrepass>,
        &Msaa,
        Has<bevy::light::VolumetricFog>,
    )>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    if *frame != 120 || std::env::var_os("MOONLY_DEBUG_CAMERA").is_none() {
        return;
    }
    for (graph, moonly, deferred, msaa, fog) in &cameras {
        info!(?graph, moonly, deferred, ?msaa, fog, "Moonly camera diagnostics");
    }
}
"#;
const COLOR_GRADIENT_ADAPTER: &str = r#"
struct MoonlyExampleCompatPlugin;

impl bevy::prelude::Plugin for MoonlyExampleCompatPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        let shader = app
            .world()
            .resource::<bevy::prelude::AssetServer>()
            .load("shaders/moonly_tonemapping_gbuffer.wesl");
        app.world_mut()
            .resource_mut::<moonly::prelude::MoonlyMaterialAdapters>()
            .set::<ColorGradientMaterial>(shader);
    }
}
"#;

struct CompatExample {
    target: &'static str,
    source: &'static str,
    sidecar: Option<&'static str>,
}

const EXAMPLES: &[CompatExample] = &[
    CompatExample {
        target: "bevy_lighting",
        source: "examples/bevy_originals/3d/lighting.rs",
        sidecar: None,
    },
    CompatExample {
        target: "bevy_spotlight",
        source: "examples/bevy_originals/3d/spotlight.rs",
        sidecar: None,
    },
    CompatExample {
        target: "bevy_transmission",
        source: "examples/bevy_originals/3d/transmission.rs",
        sidecar: None,
    },
    CompatExample {
        target: "bevy_bloom_3d",
        source: "examples/bevy_originals/3d/bloom_3d.rs",
        sidecar: None,
    },
    CompatExample {
        target: "bevy_volumetric_fog",
        source: "examples/bevy_originals/3d/volumetric_fog.rs",
        sidecar: None,
    },
    CompatExample {
        target: "bevy_tonemapping",
        source: "examples/bevy_originals/3d/tonemapping.rs",
        sidecar: None,
    },
    CompatExample {
        target: "bevy_many_lights",
        source: "examples/bevy_originals/stress_tests/many_lights.rs",
        sidecar: Some("warning_string.txt"),
    },
    CompatExample {
        target: "bevy_many_cameras_lights",
        source: "examples/bevy_originals/stress_tests/many_cameras_lights.rs",
        sidecar: Some("warning_string.txt"),
    },
];

pub(crate) fn generate() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=build/compat_examples.rs");
    let output_dir = PathBuf::from(env::var("OUT_DIR")?).join("compat_examples");
    fs::create_dir_all(&output_dir)?;
    for example in EXAMPLES {
        let source = example.source;
        println!("cargo::rerun-if-changed={source}");
        generate_example(example, &output_dir)?;
    }
    Ok(())
}

fn generate_example(example: &CompatExample, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let source_path = example.source;
    let source = fs::read_to_string(source_path)?;
    let run_index = source.rfind(".run()").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{source_path} has no .run() call"),
        )
    })?;
    let (before_run, terminal_run) = source.split_at(run_index);
    let indentation = before_run
        .rsplit_once('\n')
        .map_or(before_run, |(_, line)| line);
    let mut generated = String::with_capacity(source.len() + COMPAT_PLUGIN.len() + 1);
    generated.push_str(before_run);
    generated.push_str(COMPAT_PLUGIN);
    if example.target == "bevy_tonemapping" {
        generated.push('\n');
        generated.push_str(indentation);
        generated.push_str(MATERIAL_PLUGIN);
    }
    generated.push('\n');
    generated.push_str(indentation);
    generated.push_str(terminal_run);
    generated.push_str("\npub(crate) fn moonly_example_main() {\n    main();\n}\n");
    generated.push_str(RENDER_MODE_HELPER);
    if example.target == "bevy_tonemapping" {
        generated.push_str(COLOR_GRADIENT_ADAPTER);
    }
    let target = example.target;
    let scene_path = output_dir.join(format!("{target}_scene.rs"));
    fs::write(&scene_path, generated)?;
    let scene_path = scene_path.canonicalize()?;
    let scene_path = scene_path.to_string_lossy();
    let scene_path = scene_path.escape_default();
    let entrypoint = format!(
        "#[path = \"{scene_path}\"]\nmod original;\n\nfn main() {{\n    original::moonly_example_main();\n}}\n"
    );
    fs::write(output_dir.join(format!("{target}.rs")), entrypoint)?;
    copy_sidecar(example, output_dir)?;
    Ok(())
}

fn copy_sidecar(example: &CompatExample, output_dir: &Path) -> Result<(), Box<dyn Error>> {
    let Some(sidecar) = example.sidecar else {
        return Ok(());
    };
    let Some(source_dir) = Path::new(example.source).parent() else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "missing source directory").into());
    };
    let source_path = source_dir.join(sidecar);
    let display_path = source_path.display();
    println!("cargo::rerun-if-changed={display_path}");
    fs::copy(source_path, output_dir.join(sidecar))?;
    Ok(())
}

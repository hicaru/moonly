# Moonly Bevy Integration Guide

Moonly is a Bevy plugin for dynamic shadows, GI, occlusion, volumetrics, and HDR post-processing.

This guide covers the current API.

## 1. Requirements

Use vanilla Bevy 0.20 from crates.io. Moonly does not require a Bevy fork.

Each Moonly camera owns its intermediate scene and post-process textures. Native forward uses the standard Bevy light path.

Use one Bevy package version for the application and Moonly.

Check the dependency source with:

```bash
cargo tree -i bevy
```

## 2. Add the dependencies

Add these dependencies to the application's `Cargo.toml`:

```toml
[dependencies]
bevy = { version = "0.20", default-features = false, features = ["3d", "ui"] }
moonly = { path = "../moonly" }
```

Change the Moonly path if the repository uses a different location.

The application can enable more Bevy features when needed. Both packages must resolve to the same Bevy package version.

## 3. Add Moonly to a Bevy application

Add `MoonlyPlugin` after `DefaultPlugins`. Keep the existing Bevy scene code.

```rust
use bevy::prelude::*;
use moonly::MoonlyPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MoonlyPlugin::default())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(5.5, 4.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            intensity: 1_500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 4.6, 1.2),
    ));
}
```

The plugin automatically adapts `Camera3d`, native lights, `StandardMaterial`, bloom, tone mapping, and color grading. It preserves explicit `MoonlyCamera`, `MoonlyLight`, `MoonlySurface`, and `MoonlyPostSettings` components.

The default path uses the full Moonly renderer and R4 light attenuation. It does not replace that model with the Bevy inverse-square curve.

Add `MoonlyBevyCompatOptOut` to a `Camera3d` when that camera must keep the native Bevy render graph.

## 4. Use the native Bevy forward pipeline

Use `MoonlyNativeForward` for scenes that rely on native Bevy lighting and `StandardMaterial`.

Insert the resource **before** `MoonlyPlugin`:

```rust
use bevy::prelude::*;
use moonly::prelude::{MoonlyCamera, MoonlyNativeForward, MoonlyPlugin};

fn main() {
    let mut app = App::new();
    app.insert_resource(MoonlyNativeForward(true))
        .add_plugins(DefaultPlugins)
        .add_plugins(MoonlyPlugin::enhanced())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        MoonlyCamera::default(),
        Transform::from_xyz(0.0, 2.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
```

Keep the existing `PointLight`, `SpotLight`, `DirectionalLight`, and `StandardMaterial` components.

Native forward provides these properties:

- It preserves the Bevy `Core3d` graph.
- It preserves camera HDR, MSAA, prepasses, deferred mode, fog, and anti-aliasing.
- It uses the standard Bevy light evaluation path.
- It avoids unused Moonly subsystems.

This mode favors Bevy compatibility. It does **not** run the Moonly deferred renderer, Moonly GI, or Moonly custom shadows.

Use the full Moonly pipeline when the scene needs those features.

## 5. Configure HDR for native forward

Add `Hdr` to a native camera when the scene needs HDR post-processing.

```rust
use bevy::{camera::Hdr, prelude::*};
use moonly::prelude::MoonlyCamera;

fn spawn_hdr_camera(commands: &mut Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        MoonlyCamera::default(),
        Transform::from_xyz(0.0, 2.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
```

The full Moonly pipeline adds the HDR components that its graph requires.

## 6. Override native adaptation

`MoonlyNativeAmbient` defaults to enabled. The plugin copies `GlobalAmbientLight` into the Moonly environment.

Automatically adapted native cameras use Bevy tone mapping and native Bloom. Explicit Moonly lights and cameras keep Moonly post-processing. Insert `MoonlyNativePost(true)` before `MoonlyPlugin` to force native tone mapping for every Moonly camera.

```rust
use moonly::prelude::{MoonlyNativeAmbient, MoonlyNativePost};

app.insert_resource(MoonlyNativeAmbient(false))
    .insert_resource(MoonlyNativePost(true));
```

Explicit resource values take precedence over plugin defaults. These options do not replace `MoonlyNativeForward`.

## 7. Select a configuration preset

Use the enhanced preset for Moonly extensions:

```rust
app.add_plugins(MoonlyPlugin::enhanced());
```

Use the R4 preset for the reference-oriented configuration:

```rust
use moonly::prelude::{MoonlyConfig, MoonlyPlugin};

app.add_plugins(MoonlyPlugin {
    config: MoonlyConfig::moonly_r4(),
});
```

The enhanced preset enables features such as PCSS, SSDO, and cosine-weighted GI.

The R4 preset selects Monolith jitter shadows, HDAO, and the original GI photon count.

Change `MoonlyConfig` as a resource for runtime configuration:

```rust
fn configure(mut config: ResMut<MoonlyConfig>) {
    config.smap_pool_size = 4096;
    config.specular_quality = 1.0;
}
```

Register this system in an appropriate schedule.

## 8. Add an overlay camera

Use `MoonlyCameraRole::Overlay` for an overlay that needs independent post-processing.

```rust
use bevy::prelude::*;
use moonly::prelude::{MoonlyCamera, MoonlyCameraRole};

fn spawn_overlay(commands: &mut Commands) {
    commands.spawn((
        MoonlyCamera {
            role: MoonlyCameraRole::Overlay,
        },
        Transform::from_xyz(0.0, 2.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
```

Each Moonly camera owns private intermediate scene and post-process textures. This prevents texture feedback and separates composition spaces.

The overlay role composites its post-processed output over the world output. Both roles use the same isolated pipeline.

## 9. Control exposure and diagnostics

Use these public types for runtime controls:

- `Hdr10PostConfig` controls Moonly post-processing.
- `MoonlyExposureMode` selects the exposure source.
- `LuminanceAdaptationState` stores adaptation state.
- `MoonlyDebugView` selects a debug output.
- `MoonlyGpuDiagnostics` exposes available pass timings.
- `MoonlyLinearOutput` bypasses post-processing for linear captures.

GPU timing support depends on the adapter and backend. A pending or unavailable timestamp is not a zero-duration pass.

## 10. Validate the integration

Run these commands from the Moonly repository:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --example visual_acceptance
```

The visual acceptance command compares 16 scenes against reviewed references.

Run a scene to inspect the full pipeline:

```bash
cargo run --example monolith_showcase
cargo run --example lighting_stress_lab
```

Run the original Bevy examples with Moonly compatibility injected by the build script:

```bash
cargo run --example bevy_lighting
cargo run --example bevy_spotlight
cargo run --example bevy_transmission
cargo run --example bevy_bloom_3d
cargo run --example bevy_volumetric_fog
cargo run --example bevy_tonemapping
cargo run --example bevy_many_lights
cargo run --example bevy_many_cameras_lights
```

Enable the `exr` or `hdr` feature to load those formats in the tone-mapping image viewer.

Run the benchmark targets with the bench profile:

```bash
cargo bench --bench lighting
cargo bench --bench gi
```

Report the active profile with performance numbers. Do not use `--release` for the project workflows.

## 11. Troubleshooting

### The application has duplicate Bevy types

Use one Bevy package version for all dependencies. Run `cargo tree -i bevy` to inspect the source.

### The scene does not use Moonly

Add `MoonlyPlugin` after `DefaultPlugins`. Check that the camera does not have `MoonlyBevyCompatOptOut`.

### Native forward does not activate

Insert `MoonlyNativeForward(true)` before `MoonlyPlugin`. The plugin selects its subsystems during `Plugin::build`.

### An early capture is empty or incomplete

Let assets and GPU pipelines finish initialization before capture. Do not treat an initialization frame as a lighting regression.

### Overlay output contains feedback or the wrong post-processing

Use `MoonlyCameraRole::Overlay`. Moonly gives each camera private intermediate textures. Check that custom passes also use these textures.

### The scene needs Moonly GI and custom shadows

Do not use `MoonlyNativeForward`. Use the full Moonly pipeline and the required Moonly components.

## 12. Vanilla Bevy 0.20 compatibility

Moonly supports vanilla Bevy 0.20 without fork-only components.

Native forward omits the optional early-light optimization. The full pipeline implements texture isolation with per-camera intermediate targets.

Keep the existing Bevy camera and light setup. Do not mix different Bevy package versions in one application.

`MoonlyPlugin` includes all native adapters. `MoonlyBevyCompatPlugin` remains as a compatibility alias, but new applications do not need it.

The plugin preserves the R4 attenuation curve. Bevy uses inverse-square attenuation, so brightness can differ. Moonly reports unsupported native post options through `MoonlyBevyCompatWarnings`.

Native `Skybox`, `EnvironmentMapLight`, `VolumetricFog`, and TAA use an automatic per-camera `Core3d` fallback. Other cameras keep the Moonly pipeline. Custom `Material` types require a `MoonlyMaterialAdapters` shader.

Use these environment variables for GPU captures:

- `MOONLY_CAPTURE_PATH` selects the PNG path.
- `MOONLY_CAPTURE_FRAME` selects the capture frame.
- `MOONLY_EXIT_FRAME` selects the earliest exit frame.
- `MOONLY_INPUT_EVENTS` schedules pulse, press, and release events. Use `20:P`, `10:+Q`, and `200:-Q`.
- `MOONLY_DEBUG_VIEW` selects a raw G-buffer view. Use `scene` for linear combined output.
- `MOONLY_MAX_QUALITY=1` selects the enhanced 8192 atlas, 64-photon GI, and 256-slice volumetrics in generated Bevy examples.
- `MOONLY_VANILLA=1` selects the native Bevy forward and post path in generated Bevy examples.

The capture system waits for GPU pipeline compilation. It also waits for the PNG write before exit.

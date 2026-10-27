# Moonly Bevy Integration Guide

Moonly is a Bevy plugin for dynamic shadows, GI, occlusion, volumetrics, and HDR post-processing.

This guide covers the current API. It uses sibling `moonly` and `bevy` directories.

## 1. Requirements

Use the Bevy 0.20 development fork on the `lights` branch.

The current Moonly code uses two fork features:

- `EarlyLightCulling` skips zero-contribution lights before shadow sampling.
- `IsolatedMainTexture` gives an overlay camera private main textures.

Use one Bevy source for the application and Moonly. Do not mix the fork with another Bevy package version.

Check the dependency source with:

```bash
cargo tree -i bevy
```

## 2. Add the dependencies

Add these dependencies to the application's `Cargo.toml`:

```toml
[dependencies]
bevy = { path = "../bevy", default-features = false, features = ["3d", "ui"] }
moonly = { path = "../moonly" }
```

Change the paths if the repositories use different locations.

The application can enable more Bevy features when needed. Both packages must resolve to the same Bevy source.

## 3. Add the full Moonly pipeline

Add `MoonlyPlugin` after `DefaultPlugins`.

Use `MoonlyCamera` for each camera that Moonly must render.

```rust
use bevy::prelude::*;
use moonly::prelude::{MoonlyCamera, MoonlyLight, MoonlyPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(MoonlyPlugin::enhanced())
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        MoonlyCamera::default(),
        Transform::from_xyz(5.5, 4.5, 7.5).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));

    let position = Vec3::new(0.0, 4.6, 1.2);
    commands.spawn((
        MoonlyLight::spot(
            position,
            Vec3::new(0.0, -0.95, -0.25).normalize(),
            Vec3::new(2.8, 2.1, 1.3),
            16.0,
            1.25,
        ),
        Transform::from_translation(position),
    ));
}
```

Add meshes and materials that the scene needs. The camera and light snippets do not create visible geometry.

`MoonlyCamera` adds the required camera components and selects the Moonly render graph.

`MoonlyLight::spot` creates a shadowed spot light with GI and volumetrics enabled.

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
- It enables `EarlyLightCulling` for Moonly cameras.
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

## 6. Configure native light adaptation

The full Moonly pipeline can adapt native Bevy lights and post settings.

Insert these resources before `MoonlyPlugin` and before camera creation:

```rust
app.insert_resource(MoonlyNativeAmbient(true))
    .insert_resource(MoonlyNativePost(true));
```

Import the resources from `moonly::prelude`:

```rust
use moonly::prelude::{MoonlyNativeAmbient, MoonlyNativePost};
```

- `MoonlyNativeAmbient` copies `GlobalAmbientLight` into the Moonly environment.
- `MoonlyNativePost` keeps native Bevy tone mapping and color grading.

These options do not replace `MoonlyNativeForward`. Configure them independently.

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

The overlay role uses `IsolatedMainTexture` from the Bevy fork. This prevents texture feedback and separates composition spaces.

The world role uses the shared main texture target. This preserves the standard Moonly world pipeline.

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

Run the benchmark targets with the bench profile:

```bash
cargo bench --bench lighting
cargo bench --bench gi
```

Report the active profile with performance numbers. Do not use `--release` for the project workflows.

## 11. Troubleshooting

### The application has duplicate Bevy types

Use one Bevy path or Git source for all dependencies. Run `cargo tree -i bevy` to inspect the source.

### The scene does not use Moonly

Add `MoonlyCamera` to the camera. `Camera3d` alone does not select the Moonly render graph.

### Native forward does not activate

Insert `MoonlyNativeForward(true)` before `MoonlyPlugin`. The plugin selects its subsystems during `Plugin::build`.

### An early capture is empty or incomplete

Let assets and GPU pipelines finish initialization before capture. Do not treat an initialization frame as a lighting regression.

### Overlay output contains feedback or the wrong post-processing

Use `MoonlyCameraRole::Overlay` with the Bevy fork. For another renderer, use a separate image target and an explicit composition pass.

### The scene needs Moonly GI and custom shadows

Do not use `MoonlyNativeForward`. Use the full Moonly pipeline and the required Moonly components.

## 12. Vanilla Bevy 0.20 compatibility

Vanilla Bevy 0.20 does not provide `EarlyLightCulling` or `IsolatedMainTexture`.

The current Moonly code therefore requires adaptation before a switch to vanilla Bevy.

For vanilla integration:

- Disable or replace the early-light optimization.
- Use a separate image target for independent overlay post-processing.
- Keep the core Moonly camera and lighting setup unchanged where possible.

Do not mix vanilla Bevy types with fork Bevy types in one application.

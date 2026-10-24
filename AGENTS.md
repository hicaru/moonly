# Moonly — Agent Instructions & Coding Standards

This document defines the mandatory coding standards, architectural rules, and verification steps for the `moonly` Bevy plugin (ported and enhanced from `xray-monolith` R3/R4 lighting, dynamic shadows, GI/RT radiosity, occlusion culling, volumetrics, and HDR10 post-processing). All AI agents and contributors must follow these rules strictly.

## 1. Formatting and Linting (Strict Enforcement)

The project uses `rustfmt` and `clippy` with strict configurations (`rustfmt.toml`, `clippy.toml`, and `Cargo.toml` lints). Warnings are treated as errors.

- **Line Width:** Maximum 100 characters (`max_width = 100`).
- **Function Length:** Maximum 80 lines per function (`too-many-lines-threshold = 80`). Extract helper functions early.
- **Function Arguments:** Maximum 5 arguments (`too-many-arguments-threshold = 5`). Group related parameters into a configuration or context struct.
- **Cognitive Complexity:** Maximum 15 (`cognitive-complexity-threshold = 15`). Avoid deep nesting; use early returns and helper functions.
- **Type Complexity:** Maximum 250 (`type-complexity-threshold = 250`). Create `type` aliases or marker structs for complex Bevy `Query` types.
- **Documentation Ticks:** Enclose all code identifiers, types, and technical terms in backticks in doc comments (for example, `` `Transform` ``, `` `Vec3` ``). Exceptions: `Bevy`, `WGSL`, `WESL`, `GLTF`, `PBR`, `RGB`, `RGBA`, `3D`, `2D`, `Moonly`, `XRay`, `GI`, `RT`, `SSAO`, `SSDO`, `HDAO`, `HBAO`, `PCSS`, `RSM`, `HOM`, `SMAP`, `HDR10`.

## 2. Error Handling and Safety

- **No Panics:** `unwrap()`, `expect()`, `panic!()`, `todo!()`, and `unimplemented!()` are strictly forbidden by Clippy lints in production code.
  - Handle `Option` and `Result` gracefully using `if let`, `let ... else { return; }`, `?`, or `.unwrap_or_default()`.
  - If a Bevy query or resource lookup can fail (for example, `.single()`), use `let Ok(item) = query.single() else { return; };`.
- **Safe Indexing:** Direct slice indexing (`slice[i]`) is forbidden. Always use `.get(i)` or `.get_mut(i)` and handle the `Option`.
- **Unsafe Code:** `unsafe` blocks are allowed only when interfacing with low-level FFI or GPU APIs, and every `unsafe` block must have a `// SAFETY: ...` comment that explains why the invariants hold.

## 3. Architecture and Module Design

- **File Size Limit:** No `.rs` file may exceed **300 lines**. Split large modules into focused submodules (`mod.rs` or named files in a subdirectory).
- **Single Responsibility:** Each module must have a clear, single purpose (for example, `smap_allocator.rs` handles Guillotine shadow atlas allocation, `sun_cascades.rs` handles sun cascade matrix math, `gi_bounce.rs` handles virtual photon radiosity).
- **Explicit Public API:** Keep items private by default. Use `pub(crate)` for internal module sharing and `pub` only for APIs required by downstream crates or plugins.

## 4. Bevy Engine Conventions

- **Plugin Architecture:** Encapsulate every feature in a `Plugin`. Register components (`register_type`), events, resources, WESL shader assets, and systems inside `Plugin::build`.
- **System Design:**
  - Keep systems small (ideally under 50 lines).
  - Systems should orchestrate queries and delegate complex math or state transitions to pure, testable helper functions.
- **Query Filters:** Use marker components (`With<T>`, `Without<T>`, `Changed<T>`, `Added<T>`) instead of checking boolean flags inside loops.
- **System Ordering:** Always specify explicit ordering with `.chain()` or `SystemSet` definitions when execution order matters.
- **Render World Extraction:** Extract main-world components and resources cleanly into the render world using `ExtractResource` or `ExtractComponent`.

## 5. Memory and Performance

- **Zero Per-Frame Allocations:** Do not allocate heap memory (`Vec::new()`, `String`, `HashMap`, `Box`) inside `Update`, `PostUpdate`, or `FixedUpdate` systems unless amortized via `Local<Vec<T>>` with `.clear()`.
- **Component Storage:** Use default `Table` storage for permanent components and `#[component(storage = "SparseSet")]` for temporary or frequently toggled state tags.
- **Math Types:** Use `glam` types (`Vec3`, `Vec3A`, `Vec4`, `Quat`, `Mat4`, `Transform`) re-exported by Bevy. Prefer `Vec3A` for heavy SIMD math when alignment permits.

## 6. Code Style and Idioms

- **Early Returns:** Prefer guard clauses (`let Some(x) = opt else { return; };`) over nested `if let` blocks.
- **Iterator Chains:** Prefer idiomatic iterator adapters (`.iter().filter(...).map(...)`) over manual index loops where clarity and performance are equivalent.
- **Derive Macros:** Derive `Debug`, `Clone`, and `Reflect` (with `#[reflect(Component)]` or `#[reflect(Resource)]`) on custom Bevy components and resources.
- **Constants:** Avoid magic numbers. Define named `const` values at the top of the module with clear units in comments or names (for example, `const SUN_LONG_DIAGONAL_OFFSET_METERS: f32 = 600.0;`).

## 7. Shader Language (WESL)

- **Write all shaders in WESL:** Every shader file must use the `.wesl` extension.
- **Compile-time & test validation:** Validate all `.wesl` shader packages via `wesl` in `build.rs` and unit tests, and embed them in Bevy plugins via `embedded_asset!` or `load_shader_library!`.
- **Faithful X-Ray Monolith Port + Configurable Extensions:** Preserve exact mathematical constants and algorithms from `xray-monolith` (`R3`/`R4` sun cascades, SMAP Guillotine allocator, 32-photon GI radiosity bounce, 64x64 hierarchical occlusion rasterizer, 64x64 5-rotation jitter shadows, PCSS, MinMax shadow depth, Activision S2016 multi-bounce colored AO, HDR10 8-operator tonemapping, Karis + Kawase + dual-wave bloom, procedural lens flare, NVG, HeatVision, and parallax scope) while exposing full runtime configuration.

## 8. Verification Protocol (Mandatory Before Finishing Any Task)

After modifying any `.rs` or `.wesl` file, run the following commands in order and fix all issues before completing the task:

1. `cargo fmt --all` — Format all code according to `rustfmt.toml`.
2. `cargo clippy --workspace --all-targets -- -D warnings` — Verify zero lint warnings across all targets.
3. `cargo test --workspace` — Ensure all unit and integration tests pass.

Never leave compiler warnings, Clippy warnings, or formatting diffs unresolved.

## 9. Git Discipline

- **Atomic Commits:** Commit each file / logical change separately. Do not bundle unrelated changes into a single commit.
- **Commit Message Format:** Use conventional commits: `<type>(<scope>): <imperative description>`
  - Types: `feat`, `fix`, `refactor`, `style`, `perf`, `test`, `chore`, `docs`.
  - Keep the subject line under 72 characters.
- **Clean Commits Only:** Ensure every committed state compiles cleanly and passes `cargo fmt --check`.

## 10. Build Profiles

- **No `--release` for agents:** Build, test, benchmark, and run examples only with the dev/bench profile (`[profile.dev]` uses `opt-level = 1` and `opt-level = 3` for dependencies; `[profile.bench]` inherits `dev`). Do not pass `--release`.
- **Report profile with numbers:** Always specify the build profile alongside any reported benchmark or frame-time metric.

## 11. Comments & Prose (ASD-STE100)

Write all code comments, doc comments, commit messages, and documentation in **Simplified Technical English (ASD-STE100)**:

- **Sentence Length:** Maximum **20 words** for procedural instructions; maximum **25 words** for descriptive text.
- **One Idea Per Sentence:** Express one thought or action per sentence.
- **Active Voice Only:** Write `The allocator splits the free quad.` instead of `The free quad is split by the allocator.`
- **Allowed Tenses Only:** Use only Simple Present, Simple Past, Simple Future, or Imperative. Avoid `-ing` verb forms (gerunds and participles) in comments and documentation.
- **Concise Explanations:** Explain **why** a non-obvious decision exists, not **what** the syntax already says.

//! Build-time WESL shader validation for the `moonly` workspace.

use std::error::Error;

use wesl::VirtualResolver;

const WESL_PACKAGES: &[(&str, &str, &str)] = &[
    (
        "crates/moonly_render/src/depth_copy.wesl",
        "crates/moonly_render/src",
        "package::depth_copy",
    ),
    (
        "crates/moonly_render/src/froxel.wesl",
        "crates/moonly_render/src",
        "package::froxel",
    ),
    (
        "crates/moonly_core/src/core_common.wesl",
        "crates/moonly_core/src",
        "package::core_common",
    ),
    (
        "crates/moonly_shadows/src/shadows.wesl",
        "crates/moonly_shadows/src",
        "package::shadows",
    ),
    (
        "crates/moonly_gi/src/gi.wesl",
        "crates/moonly_gi/src",
        "package::gi",
    ),
    (
        "crates/moonly_volumetrics/src/volumetrics.wesl",
        "crates/moonly_volumetrics/src",
        "package::volumetrics",
    ),
    (
        "crates/moonly_post/src/tonemap.wesl",
        "crates/moonly_post/src",
        "package::tonemap",
    ),
    (
        "crates/moonly_post/src/bloom_optics.wesl",
        "crates/moonly_post/src",
        "package::bloom_optics",
    ),
];

const BEVY_FORWARD_IO: &str = r"
struct Vertex { @builtin(instance_index) instance_index: u32 }
struct DecompressedVertex { position: vec3<f32>, normal: vec3<f32> }
fn decompress_vertex(v: Vertex, instance: u32) -> DecompressedVertex {
    var out: DecompressedVertex;
    return out;
}
";
const BEVY_MESH_FUNCTIONS: &str = r"
fn get_world_from_local(instance: u32) -> mat4x4<f32> { return mat4x4<f32>(); }
fn mesh_normal_local_to_world(normal: vec3<f32>, instance: u32) -> vec3<f32> { return normal; }
fn mesh_position_local_to_world(world: mat4x4<f32>, position: vec4<f32>) -> vec4<f32> {
    return world * position;
}
";
const BEVY_VIEW_BINDINGS: &str = r"
struct View { world_position: vec3<f32> }
@group(0) @binding(0) var<uniform> view: View;
";
const BEVY_VIEW_TRANSFORMS: &str = r"
fn frag_coord_to_uv(position: vec2<f32>) -> vec2<f32> { return position; }
fn position_world_to_clip(position: vec3<f32>) -> vec4<f32> { return vec4<f32>(position, 1.0); }
";
const BEVY_PREPASS_IO: &str = r"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(4) world_position: vec4<f32>,
}
";
const BEVY_PBR_TYPES: &str = r"
struct StandardMaterial {
    base_color: vec4<f32>,
    alpha_cutoff: f32,
    flags: u32,
    perceptual_roughness: f32,
    metallic: f32,
    emissive: vec4<f32>,
}
struct PbrInput { material: StandardMaterial, N: vec3<f32> }
";
const BEVY_PBR_FRAGMENT: &str = r"
import bevy_pbr::render::pbr_types::PbrInput;
fn pbr_input_from_standard_material(input: VertexOutput, front: bool) -> PbrInput {
    var out: PbrInput;
    return out;
}
";
const BEVY_PBR_FUNCTIONS: &str = r"
fn alpha_discard(flags: u32, cutoff: f32, color: vec4<f32>) -> vec4<f32> { return color; }
";
const BEVY_PBR_LIGHTING: &str = r"
fn D_GGX(roughness: f32, NdotH: f32) -> f32 { return 0.0; }
fn V_SmithGGXCorrelated(roughness: f32, NdotV: f32, NdotL: f32) -> f32 { return 0.0; }
fn F_Schlick(f0: f32, f90: f32, VdotH: f32) -> f32 { return 0.0; }
fn fresnel(f0: vec3<f32>, LdotH: f32) -> vec3<f32> { return vec3<f32>(); }
fn specular_multiscatter(
    D: f32,
    V: f32,
    F: vec3<f32>,
    F0: vec3<f32>,
    F_ab: vec2<f32>,
    specular_intensity: f32,
) -> vec3<f32> { return vec3<f32>(); }
";

fn main() -> Result<(), Box<dyn Error>> {
    for &(file_path, base_dir, module_path) in WESL_PACKAGES {
        println!("cargo::rerun-if-changed={file_path}");
        let parsed = module_path.parse()?;
        wesl::Wesl::new(base_dir).compile(&parsed)?;
    }
    compile_lit_material()?;
    compile_gbuffer()?;
    compile_deferred_lighting()?;
    println!("cargo::rerun-if-changed=crates/moonly_render/src/combine.wesl");
    wesl::Wesl::new("crates/moonly_render/src")
        .add_constant("MOONLY_DEBUG_VIEW", 0.0)
        .add_constant("MOONLY_OVERLAY", 0.0)
        .compile(&"package::combine".parse()?)?;
    Ok(())
}

fn compile_lit_material() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=crates/moonly_post/src/moonly_lit.wesl");
    let mut resolver = VirtualResolver::new();
    add_module(
        &mut resolver,
        "package::moonly_lit",
        include_str!("crates/moonly_post/src/moonly_lit.wesl"),
    )?;
    add_module(
        &mut resolver,
        "package::tonemap",
        include_str!("crates/moonly_post/src/tonemap.wesl"),
    )?;
    add_module(
        &mut resolver,
        "package::bloom_optics",
        include_str!("crates/moonly_post/src/bloom_optics.wesl"),
    )?;
    add_module(
        &mut resolver,
        "bevy_pbr::render::forward_io",
        BEVY_FORWARD_IO,
    )?;
    add_module(
        &mut resolver,
        "bevy_pbr::render::mesh_functions",
        BEVY_MESH_FUNCTIONS,
    )?;
    add_module(
        &mut resolver,
        "bevy_pbr::render::mesh_view_bindings",
        BEVY_VIEW_BINDINGS,
    )?;
    add_module(
        &mut resolver,
        "bevy_pbr::render::view_transformations",
        BEVY_VIEW_TRANSFORMS,
    )?;
    wesl::Wesl::new_barebones()
        .set_custom_resolver(resolver)
        .compile(&"package::moonly_lit".parse()?)?;
    Ok(())
}

fn compile_gbuffer() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=crates/moonly_render/src/gbuffer.wesl");
    println!("cargo::rerun-if-changed=crates/moonly_render/src/gbuffer_surface.wesl");
    let mut resolver = VirtualResolver::new();
    add_module(
        &mut resolver,
        "package::gbuffer",
        include_str!("crates/moonly_render/src/gbuffer.wesl"),
    )?;
    add_module(
        &mut resolver,
        "package::gbuffer_surface",
        include_str!("crates/moonly_render/src/gbuffer_surface.wesl"),
    )?;
    add_module(&mut resolver, "bevy_pbr::prepass::io", BEVY_PREPASS_IO)?;
    add_module(&mut resolver, "bevy_pbr::render::pbr_types", BEVY_PBR_TYPES)?;
    add_module(
        &mut resolver,
        "bevy_pbr::render::pbr_fragment",
        BEVY_PBR_FRAGMENT,
    )?;
    add_module(
        &mut resolver,
        "bevy_pbr::render::pbr_functions",
        BEVY_PBR_FUNCTIONS,
    )?;
    wesl::Wesl::new_barebones()
        .set_custom_resolver(resolver)
        .compile(&"package::gbuffer".parse()?)?;
    Ok(())
}

fn compile_deferred_lighting() -> Result<(), Box<dyn Error>> {
    let mut resolver = VirtualResolver::new();
    for (path, source, file) in [
        (
            "package::lighting",
            include_str!("crates/moonly_render/src/lighting.wesl"),
            "crates/moonly_render/src/lighting.wesl",
        ),
        (
            "package::accum_spot",
            include_str!("crates/moonly_render/src/accum_spot.wesl"),
            "crates/moonly_render/src/accum_spot.wesl",
        ),
        (
            "package::accum_direct",
            include_str!("crates/moonly_render/src/accum_direct.wesl"),
            "crates/moonly_render/src/accum_direct.wesl",
        ),
    ] {
        println!("cargo::rerun-if-changed={file}");
        add_module(&mut resolver, path, source)?;
    }
    add_module(
        &mut resolver,
        "bevy_pbr::render::pbr_lighting",
        BEVY_PBR_LIGHTING,
    )?;
    wesl::Wesl::new_barebones()
        .set_custom_resolver(resolver)
        .compile(&"package::accum_spot".parse()?)?;
    Ok(())
}

fn add_module(
    resolver: &mut VirtualResolver<'static>,
    path: &str,
    source: &'static str,
) -> Result<(), Box<dyn Error>> {
    resolver.add_module(path.parse()?, source.into());
    Ok(())
}

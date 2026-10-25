import bevy_pbr::{
    prepass::io::VertexOutput,
    render::{
        pbr_fragment::pbr_input_from_standard_material,
        pbr_functions::alpha_discard,
    },
};

struct MoonlySurfaceUniform {
    values: vec2<f32>,
    positive_cube: vec4<f32>,
    negative_cube: vec4<f32>,
    local: vec4<f32>,
    ambient: vec4<f32>,
    hemi: vec4<f32>,
};

@group(4) @binding(0) var<uniform> surface: MoonlySurfaceUniform;

struct FragmentOutput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) albedo_gloss: vec4<f32>,
    @location(3) accumulator: vec4<f32>,
};

fn tracked_irradiance(normal: vec3<f32>) -> vec3<f32> {
    let positive = max(normal.x, 0.0) * surface.positive_cube.x
        + max(normal.y, 0.0) * surface.positive_cube.y
        + max(normal.z, 0.0) * surface.positive_cube.z;
    let negative = max(-normal.x, 0.0) * surface.negative_cube.x
        + max(-normal.y, 0.0) * surface.negative_cube.y
        + max(-normal.z, 0.0) * surface.negative_cube.z;
    let cube = max(positive + negative, 1.0 / 255.0);
    let hemi = surface.hemi.rgb * surface.hemi.a
        * (surface.positive_cube.w * 0.5 + cube * 0.25) * 0.75;
    return surface.ambient.rgb + hemi + surface.local.rgb;
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );

    let normal = normalize(pbr_input.N);
    let wetness = clamp(surface.values.y * max(normal.y, 0.0), 0.0, 1.0);
    let albedo = pbr_input.material.base_color.rgb * mix(1.0, 0.58, wetness);
    let gloss = mix(1.0 - pbr_input.material.perceptual_roughness, 0.92, wetness);
    var out: FragmentOutput;
    out.position = vec4(in.world_position.xyz, 1.0);
    out.normal = vec4(normal, surface.values.x);
    out.albedo_gloss = vec4(albedo, gloss);
    let indirect = tracked_irradiance(normal) * albedo * surface.local.a;
    out.accumulator = vec4(pbr_input.material.emissive.rgb + indirect, 1.0 + surface.local.a);
    return out;
}

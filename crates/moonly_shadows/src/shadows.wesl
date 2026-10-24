// XRay-Monolith R3/R4 shadow filtering, 64x64 jitter rotation, MinMax SM, and PCSS in WESL.

const JITTER_TILE_SIZE: f32 = 64.0;
const TWO_PI: f32 = 6.283185307179586;

// Procedural 64x64 5-rotation jitter vector matching `TEX_jitter` (`r4_rendertarget.cpp`).
fn sample_monolith_jitter(screen_xy: vec2<f32>, rotation_idx: u32) -> vec2<f32> {
    let cell = floor(screen_xy % vec2<f32>(JITTER_TILE_SIZE));
    let phase = f32(rotation_idx) * 1.25663706;
    let seed = fract(sin(dot(cell, vec2<f32>(12.9898, 78.233)) + phase) * 43758.5453);
    let angle = seed * TWO_PI;
    return vec2<f32>(cos(angle), sin(angle));
}

// Evaluate 4x4 MinMax shadow block (`r4_rendertarget_create_minmaxSM.cpp`).
// `minmax_z.x` is min depth (closest blocker), `minmax_z.y` is max depth (farthest blocker).
// Returns: 1.0 = fully lit, 0.0 = fully shadowed, -1.0 = penumbra (requires full filter).
fn test_minmax_shadow_block(receiver_z: f32, minmax_z: vec2<f32>, bias: f32) -> f32 {
    let z = receiver_z + bias;
    if z <= minmax_z.x {
        return 1.0;
    }
    if z > minmax_z.y {
        return 0.0;
    }
    return -1.0;
}

// Analytical 5-rotation jittered shadow filter for a virtual shadow caster plane.
fn filter_monolith_jitter64(
    shadow_uv: vec2<f32>,
    receiver_z: f32,
    blocker_z: f32,
    screen_xy: vec2<f32>,
    texel_radius: f32,
) -> f32 {
    var acc = 0.0;
    for (var rot: u32 = 0u; rot < 5u; rot = rot + 1u) {
        let j = sample_monolith_jitter(screen_xy, rot) * texel_radius;
        let sample_uv = shadow_uv + j;
        let edge_dist = length(sample_uv - vec2<f32>(0.5));
        let sample_blocker = blocker_z + edge_dist * 0.015;
        let lit = select(0.0, 1.0, receiver_z <= sample_blocker);
        acc = acc + lit;
    }
    return acc * 0.2;
}

// Percentage-Closer Soft Shadows (PCSS) penumbra width calculation.
fn pcss_penumbra_width(receiver_z: f32, avg_blocker_z: f32, light_size: f32) -> f32 {
    let d_blocker = max(avg_blocker_z, 0.001);
    let d_receiver = max(receiver_z, d_blocker);
    return ((d_receiver - d_blocker) / d_blocker) * light_size;
}

// Wet-surface specular and diffuse modification from `r4_R_rain.cpp`.
fn apply_rain_wetness(
    albedo: vec3<f32>,
    roughness: f32,
    world_normal: vec3<f32>,
    wetness: f32,
) -> vec4<f32> {
    let up_facing = clamp(world_normal.y, 0.0, 1.0);
    let effective_wet = clamp(wetness * up_facing, 0.0, 1.0);
    let darkened_albedo = albedo * mix(1.0, 0.58, effective_wet);
    let wet_roughness = mix(roughness, 0.08, effective_wet);
    return vec4<f32>(darkened_albedo, wet_roughness);
}

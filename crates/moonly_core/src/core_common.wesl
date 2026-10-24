// XRay-Monolith R3/R4 common shader math ported to WESL (`common_functions.h`).

const DEF_GLOSS: f32 = 0.25;
const INV_SQRT_3: f32 = 0.5773502691896258;
const REC709_LUMA: vec3<f32> = vec3<f32>(0.212656, 0.715158, 0.072186);

struct ShaderBusUniform {
    params: array<vec4<f32>, 8>,
}

fn calc_fogging(w_pos: vec4<f32>, fog_plane: vec4<f32>) -> f32 {
    return clamp(dot(w_pos, fog_plane), 0.0, 1.0);
}

fn u_diffuse2s(rgb: vec3<f32>) -> f32 {
    return length(rgb) * INV_SQRT_3;
}

fn tonemap_xray(rgb: vec3<f32>, scale: f32) -> vec3<f32> {
    let c = rgb * scale;
    return (c * (vec3<f32>(1.0) + c / 2.89)) / (c + vec3<f32>(1.0));
}

// Activision S2016 multi-bounce colored ambient occlusion (`common_functions.h`).
fn compute_colored_ao(ao: f32, albedo: vec3<f32>) -> vec3<f32> {
    let a = 2.0404 * albedo - vec3<f32>(0.3324);
    let b = -4.7951 * albedo + vec3<f32>(0.6417);
    let c = 2.7552 * albedo + vec3<f32>(0.6903);
    let poly = ((vec3<f32>(ao) * a + b) * ao + c) * ao;
    return max(vec3<f32>(ao), poly);
}

// Piecewise power contrast curve (`common_functions.h`).
fn contrast_curve(input: vec3<f32>, contrast_val: f32) -> vec3<f32> {
    let p = pow(2.0, contrast_val);
    let lo = 0.5 * pow(max(2.0 * input, vec3<f32>(0.0)), vec3<f32>(p));
    let hi = vec3<f32>(1.0) - 0.5 * pow(max(2.0 * (vec3<f32>(1.0) - input), vec3<f32>(0.0)), vec3<f32>(p));
    return select(hi, lo, input < vec3<f32>(0.5));
}

// Saturation-aware vibrance adjustment (`common_functions.h`).
fn vibrance_boost(color: vec3<f32>, val: f32) -> vec3<f32> {
    let luma = dot(color, REC709_LUMA);
    let max_c = max(color.r, max(color.g, color.b));
    let min_c = min(color.r, min(color.g, color.b));
    let sat = max_c - min_c;
    let boost = 1.0 + (val * (1.0 - (sign(val) * sat)));
    return mix(vec3<f32>(luma), color, boost);
}

// Hoskins sinless hash functions (`common_functions.h`).
fn hash12(p_in: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p_in.xyx) * 0.1031);
    p3 = p3 + vec3<f32>(dot(p3, p3.yzx + vec3<f32>(33.33)));
    return fract((p3.x + p3.y) * p3.z);
}

fn hash22(p_in: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p_in.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + vec3<f32>(dot(p3, p3.yzx + vec3<f32>(33.33)));
    return fract((p3.xx + p3.yz) * p3.zy);
}

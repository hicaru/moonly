// XRay-Monolith R4 virtual GI bounce accumulation, HDAO, SSDO, and Activision colored AO in WESL.

struct VirtualGiLightGpu {
    position_range: vec4<f32>,
    direction_pad: vec4<f32>,
    color_spec: vec4<f32>,
}

// Activision S2016 multi-bounce colored AO (`common_functions.h`).
fn multi_bounce_colored_ao(ao: f32, albedo: vec3<f32>) -> vec3<f32> {
    let a = 2.0404 * albedo - vec3<f32>(0.3324);
    let b = -4.7951 * albedo + vec3<f32>(0.6417);
    let c = 2.7552 * albedo + vec3<f32>(0.6903);
    let poly = ((vec3<f32>(ao) * a + b) * ao + c) * ao;
    return max(vec3<f32>(ao), poly);
}

// Evaluate one virtual indirect bounce light (`r4_rendertarget_accum_reflected.cpp`).
fn eval_virtual_gi_light(
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    view_dir: vec3<f32>,
    vlight: VirtualGiLightGpu,
    gloss: f32,
) -> vec4<f32> {
    let to_light = vlight.position_range.xyz - world_pos;
    let range = max(vlight.position_range.w, 0.001);
    let dist = length(to_light);
    if dist >= range || dist < 0.0001 {
        return vec4<f32>(0.0);
    }
    let l_dir = to_light / dist;
    let ndotl = max(dot(world_normal, l_dir), 0.0);
    if ndotl <= 0.0 {
        return vec4<f32>(0.0);
    }
    let emit_cos = max(dot(vlight.direction_pad.xyz, -l_dir), 0.15);
    let atten = clamp(1.0 - dist / range, 0.0, 1.0);
    let falloff = atten * atten * emit_cos;
    let half_vec = normalize(l_dir + view_dir);
    let spec = pow(max(dot(world_normal, half_vec), 0.0), 16.0) * gloss * vlight.color_spec.w;
    let rgb = vlight.color_spec.xyz * (ndotl * falloff);
    return vec4<f32>(rgb, spec * falloff);
}

// High-Definition Ambient Occlusion valley test (`r4_rendertarget_phase_hdao.cpp`).
fn eval_hdao_valley(
    center_normal: vec3<f32>,
    offset_dir: vec3<f32>,
    dist: f32,
    accept_radius: f32,
    reject_radius: f32,
) -> f32 {
    if dist < accept_radius || dist > reject_radius {
        return 1.0;
    }
    let valley = clamp(dot(center_normal, offset_dir), 0.0, 1.0);
    let falloff = 1.0 - smoothstep(accept_radius, reject_radius, dist);
    return 1.0 - valley * falloff;
}

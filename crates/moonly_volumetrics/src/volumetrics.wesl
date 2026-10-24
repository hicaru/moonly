// XRay-Monolith R4 volumetric sunshafts and spot cone raymarching in WESL.

const INV_FOUR_PI: f32 = 0.07957747154594767;

fn henyey_greenstein_wesl(cos_theta: f32, g: f32) -> f32 {
    let g_c = clamp(g, -0.95, 0.95);
    let g2 = g_c * g_c;
    let denom = pow(max(1.0 + g2 - 2.0 * g_c * cos_theta, 0.0001), 1.5);
    return (1.0 - g2) * INV_FOUR_PI / denom;
}

// Jittered raymarch for sunshafts (`accum_direct_volumetric` in `r4_rendertarget_accum_direct.cpp`).
fn raymarch_volumetric_sunshafts(
    camera_pos: vec3<f32>,
    world_pos: vec3<f32>,
    sun_dir: vec3<f32>,
    sun_color: vec3<f32>,
    params: vec4<f32>, // x = density, y = anisotropy_g, z = jitter_offset, w = step_count
) -> vec3<f32> {
    let ray_vec = world_pos - camera_pos;
    let max_dist = min(length(ray_vec), 80.0);
    if max_dist <= 0.001 {
        return vec3<f32>(0.0);
    }
    let view_dir = ray_vec / max_dist;
    let cos_theta = dot(view_dir, -normalize(sun_dir));
    let phase = henyey_greenstein_wesl(cos_theta, params.y);

    let steps = u32(clamp(params.w, 4.0, 48.0));
    let step_len = max_dist / f32(steps);
    var optical_depth = 0.0;
    for (var i: u32 = 0u; i < steps; i = i + 1u) {
        let t = (f32(i) + fract(params.z)) * step_len;
        let sample_pos = camera_pos + view_dir * t;
        let height_fog = exp(-0.08 * max(sample_pos.y, 0.0));
        optical_depth = optical_depth + height_fog * step_len;
    }
    return sun_color * (optical_depth * phase * params.x * 0.05);
}

// Volumetric spot cone scattering (`accum_volumetric` in `r4_rendertarget_accum_spot.cpp`).
fn eval_volumetric_spot_cone(
    world_pos: vec3<f32>,
    spot_pos: vec3<f32>,
    spot_dir: vec3<f32>,
    spot_color: vec3<f32>,
    cone_params: vec3<f32>, // x = range, y = cos_outer_cone, z = intensity
) -> vec3<f32> {
    let to_pt = world_pos - spot_pos;
    let dist = length(to_pt);
    let range = max(cone_params.x, 0.01);
    if dist >= range || dist <= 0.001 {
        return vec3<f32>(0.0);
    }
    let dir = to_pt / dist;
    let cos_ang = dot(dir, normalize(spot_dir));
    if cos_ang <= cone_params.y {
        return vec3<f32>(0.0);
    }
    let cone_atten = smoothstep(cone_params.y, min(cone_params.y + 0.15, 1.0), cos_ang);
    let dist_atten = 1.0 - dist / range;
    return spot_color * (cone_atten * dist_atten * dist_atten * cone_params.z);
}

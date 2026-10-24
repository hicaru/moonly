// XRay-Monolith R4 Karis bloom, lens flare, NightVision, HeatVision, and ParallaxScope in WESL.

fn karis_weight_wesl(rgb: vec3<f32>) -> f32 {
    let luma = max(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0);
    return 1.0 / (1.0 + luma);
}

fn karis_box4_wesl(c0: vec3<f32>, c1: vec3<f32>, c2: vec3<f32>, c3: vec3<f32>) -> vec3<f32> {
    let avg = (c0 + c1 + c2 + c3) * 0.25;
    return avg * karis_weight_wesl(avg);
}

fn eval_lens_flare_wesl(
    uv: vec2<f32>,
    light_uv: vec2<f32>,
    dispersal: f32,
    halo_radius: f32,
) -> vec3<f32> {
    let flipped = vec2<f32>(1.0) - uv;
    let to_center = vec2<f32>(0.5) - flipped;
    var acc = vec3<f32>(0.0);
    for (var i: u32 = 1u; i <= 4u; i = i + 1u) {
        let ghost_uv = flipped + to_center * (f32(i) * dispersal);
        let d = distance(ghost_uv, light_uv);
        let w = pow(1.0 - clamp(d * 4.5, 0.0, 1.0), 3.0);
        acc = acc + vec3<f32>(w * 1.1, w, w * 0.9) * 0.25;
    }
    let dir_len = max(length(to_center), 0.0001);
    let halo_vec = (to_center / dir_len) * halo_radius;
    let halo_d = distance(flipped + halo_vec, vec2<f32>(0.5));
    let halo_w = pow(1.0 - clamp(abs(halo_d - halo_radius) * 8.0, 0.0, 1.0), 4.0);
    return acc + vec3<f32>(0.8, 0.9, 1.1) * (halo_w * 0.35);
}

fn apply_night_vision_wesl(linear_rgb: vec3<f32>, uv: vec2<f32>, white_phosphor: bool) -> vec3<f32> {
    let luma = max(dot(linear_rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0);
    let amp = clamp(pow(luma * 4.5, 0.75), 0.0, 1.5);
    let d = length(uv - vec2<f32>(0.5));
    let vig = pow(1.0 - clamp((d - 0.38) / 0.14, 0.0, 1.0), 2.0);
    let scanline = 0.94 + 0.06 * sin(uv.y * 540.0);
    let phosphor = select(
        vec3<f32>(0.18, 0.96, 0.28),
        vec3<f32>(0.68, 0.92, 0.98),
        white_phosphor,
    );
    return phosphor * (amp * vig * scanline);
}

fn apply_heat_vision_wesl(linear_rgb: vec3<f32>) -> vec3<f32> {
    let heat = clamp(dot(linear_rgb, vec3<f32>(0.299, 0.587, 0.114)), 0.0, 1.0);
    let cold = vec3<f32>(0.05, 0.08, 0.35);
    let mid = vec3<f32>(0.78, 0.14, 0.52);
    let hot = vec3<f32>(1.0, 0.92, 0.25);
    return select(
        mix(mid, hot, (heat - 0.5) * 2.0),
        mix(cold, mid, heat * 2.0),
        heat < 0.5,
    );
}

fn apply_parallax_scope_wesl(color: vec3<f32>, uv: vec2<f32>, parallax_offset: vec2<f32>) -> vec3<f32> {
    let shifted = uv - vec2<f32>(0.5) - parallax_offset;
    let r = length(shifted);
    let rim = smoothstep(0.42, 0.39, r);
    let cross_x = smoothstep(0.0025, 0.0008, abs(shifted.x));
    let cross_y = smoothstep(0.0025, 0.0008, abs(shifted.y));
    let reticle = max(cross_x, cross_y) * step(r, 0.39);
    let tinted = color * vec3<f32>(0.94, 1.0, 0.96);
    return mix(tinted, vec3<f32>(0.05, 0.05, 0.05), reticle * 0.85) * rim;
}

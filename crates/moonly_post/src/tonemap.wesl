// XRay-Monolith R4 HDR10 tonemapping operators, color spaces, and ST.2084 PQ in WESL (`hdr10.h`).

fn tonemap_reinhard_wp(x: vec3<f32>, wp_sq: f32) -> vec3<f32> {
    let mapped = (x * (vec3<f32>(1.0) + x / wp_sq)) / (x + vec3<f32>(1.0));
    return clamp(mapped, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap_aces_narkowicz(x: vec3<f32>) -> vec3<f32> {
    let num = x * (2.51 * x + vec3<f32>(0.03));
    let den = x * (2.43 * x + vec3<f32>(0.59)) + vec3<f32>(0.14);
    return clamp(num / den, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap_aces_hill(x: vec3<f32>) -> vec3<f32> {
    let v = vec3<f32>(
        0.59719 * x.x + 0.35458 * x.y + 0.04823 * x.z,
        0.07600 * x.x + 0.90834 * x.y + 0.01566 * x.z,
        0.02840 * x.x + 0.13383 * x.y + 0.83777 * x.z,
    );
    let a = v * (v + vec3<f32>(0.0245786)) - vec3<f32>(0.000090537);
    let b = v * (0.983729 * v + vec3<f32>(0.4329510)) + vec3<f32>(0.238081);
    let rrt = a / b;
    let out = vec3<f32>(
        1.60475 * rrt.x - 0.53108 * rrt.y - 0.07367 * rrt.z,
        -0.10208 * rrt.x + 1.10813 * rrt.y - 0.00605 * rrt.z,
        -0.00327 * rrt.x - 0.07276 * rrt.y + 1.07602 * rrt.z,
    );
    return clamp(out, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap_uchimura_scalar(x: f32) -> f32 {
    let p = 1.0;
    let a = 1.0;
    let m = 0.22;
    let l = 0.4;
    let c = 1.33;
    let l0 = ((p - m) * l) / a;
    let s0 = m + l0;
    let s1 = m + a * l0;
    let c2 = (a * p) / (p - s1);
    let cp = -c2 / p;
    let w0 = 1.0 - smoothstep(0.0, m, x);
    let w2 = select(0.0, 1.0, x >= m + l0);
    let w1 = 1.0 - w0 - w2;
    let t = m * pow(max(x / m, 0.0), c);
    let s = p - (p - s1) * exp(cp * (x - s0));
    let linear = m + a * (x - m);
    return clamp(t * w0 + linear * w1 + s * w2, 0.0, 1.0);
}

fn tonemap_uchimura(x: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        tonemap_uchimura_scalar(x.x),
        tonemap_uchimura_scalar(x.y),
        tonemap_uchimura_scalar(x.z),
    );
}

fn tonemap_stevem(x: vec3<f32>) -> vec3<f32> {
    let mapped = (x * (10.0 * x + vec3<f32>(0.3)))
        / (x * (10.0 * x + vec3<f32>(0.5)) + vec3<f32>(1.5));
    return clamp(mapped, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn uncharted2_partial_wesl(x: vec3<f32>) -> vec3<f32> {
    let a = 0.15;
    let b = 0.50;
    let c = 0.10;
    let d = 0.20;
    let e = 0.02;
    let f = 0.30;
    return ((x * (a * x + vec3<f32>(c * b)) + vec3<f32>(d * e))
        / (x * (a * x + vec3<f32>(b)) + vec3<f32>(d * f)))
        - vec3<f32>(e / f);
}

fn tonemap_uncharted2(x: vec3<f32>) -> vec3<f32> {
    let curr = uncharted2_partial_wesl(x * 2.0);
    let w_scale = vec3<f32>(1.0) / uncharted2_partial_wesl(vec3<f32>(11.2));
    return clamp(curr * w_scale, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tonemap_agx(x: vec3<f32>) -> vec3<f32> {
    let inset = max(
        vec3<f32>(
            0.84247906 * x.x + 0.0784336 * x.y + 0.07922375 * x.z,
            0.04232824 * x.x + 0.87846864 * x.y + 0.07916613 * x.z,
            0.04237565 * x.x + 0.0784336 * x.y + 0.87914297 * x.z,
        ),
        vec3<f32>(1.0e-10),
    );
    let ev = log2(inset);
    let t = clamp((ev - vec3<f32>(-12.47393)) / (4.026069 - (-12.47393)), vec3<f32>(0.0), vec3<f32>(1.0));
    let t2 = t * t;
    let t4 = t2 * t2;
    let sig = 15.5 * t4 * t2 - 40.14 * t4 * t + 31.96 * t4 - 6.868 * t2 * t + 0.4298 * t2 + 0.1191 * t - vec3<f32>(0.00232);
    let out = vec3<f32>(
        1.196879 * sig.x - 0.09802088 * sig.y - 0.09902975 * sig.z,
        -0.05289685 * sig.x + 1.1519031 * sig.y - 0.09896118 * sig.z,
        -0.05297163 * sig.x - 0.09804345 * sig.y + 1.1510736 * sig.z,
    );
    return clamp(pow(max(out, vec3<f32>(0.0)), vec3<f32>(2.2)), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn evaluate_hdr10_tonemap(rgb: vec3<f32>, op_idx: u32) -> vec3<f32> {
    let c = max(rgb, vec3<f32>(0.0));
    if op_idx == 0u {
        return tonemap_reinhard_wp(c, 2.89);
    }
    if op_idx == 1u {
        return tonemap_reinhard_wp(c, 4.0);
    }
    if op_idx == 2u {
        return tonemap_reinhard_wp(c, 9.0);
    }
    if op_idx == 3u {
        return tonemap_aces_narkowicz(c);
    }
    if op_idx == 4u {
        return tonemap_aces_hill(c);
    }
    if op_idx == 5u {
        return tonemap_uchimura(c);
    }
    if op_idx == 6u {
        return tonemap_stevem(c);
    }
    if op_idx == 7u {
        return tonemap_uncharted2(c);
    }
    if op_idx == 8u {
        return tonemap_agx(c);
    }
    return tonemap_aces_hill(c);
}

fn encode_st2084_pq(linear_rgb: vec3<f32>, max_nits: f32) -> vec3<f32> {
    let m1 = 0.1593017578125;
    let m2 = 78.84375;
    let c1 = 0.8359375;
    let c2 = 18.8515625;
    let c3 = 18.6875;
    let y = clamp(linear_rgb * (max_nits / 10000.0), vec3<f32>(0.0), vec3<f32>(1.0));
    let ym1 = pow(y, vec3<f32>(m1));
    let num = vec3<f32>(c1) + c2 * ym1;
    let den = vec3<f32>(1.0) + c3 * ym1;
    return pow(num / den, vec3<f32>(m2));
}

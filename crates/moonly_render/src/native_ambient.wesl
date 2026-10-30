fn environment_brdf(f0: vec3<f32>, f_ab: vec2<f32>) -> vec3<f32> {
    return f0 * f_ab.x + f_ab.y;
}

fn environment_f_ab(perceptual_roughness: f32, n_dot_v: f32) -> vec2<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = perceptual_roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    return max(
        vec2<f32>(-1.04, 1.04) * a004 + r.zw,
        vec2<f32>(0.00005),
    );
}

fn evaluate_native_ambient(
    normal: vec3<f32>,
    view_direction: vec3<f32>,
    base_color: vec3<f32>,
    metallic: f32,
    perceptual_roughness: f32,
    ambient_color: vec3<f32>,
) -> vec3<f32> {
    let n_dot_v = max(dot(normal, view_direction), 0.0001);
    let diffuse_color = base_color * (1.0 - metallic);
    let f0 = mix(vec3(0.04), base_color, metallic);
    let diffuse = environment_brdf(diffuse_color, environment_f_ab(1.0, n_dot_v));
    let specular = environment_brdf(f0, environment_f_ab(perceptual_roughness, n_dot_v));
    let specular_occlusion = clamp(dot(f0, vec3(50.0 * 0.33)), 0.0, 1.0);
    return (diffuse + specular * specular_occlusion) * ambient_color;
}

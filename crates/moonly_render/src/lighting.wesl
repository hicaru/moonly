import bevy_pbr::render::pbr_lighting::{
    D_GGX,
    V_SmithGGXCorrelated,
    F_Schlick,
    fresnel,
    specular_multiscatter,
};

const MOONLY_INVERSE_PI: f32 = 0.3183098861837907;

fn moonly_f_ab(perceptual_roughness: f32, NdotV: f32) -> vec2<f32> {
    let c0 = vec4<f32>(-1.0, -0.0275, -0.572, 0.022);
    let c1 = vec4<f32>(1.0, 0.0425, 1.04, -0.04);
    let r = perceptual_roughness * c0 + c1;
    let a004 = min(r.x * r.x, exp2(-9.28 * NdotV)) * r.x + r.y;
    return vec2<f32>(-1.04, 1.04) * a004 + r.zw;
}

fn moonly_brdf(
    normal: vec3<f32>,
    view_direction: vec3<f32>,
    light_direction: vec3<f32>,
    base_color: vec3<f32>,
    metallic: f32,
    perceptual_roughness: f32,
) -> vec3<f32> {
    let NdotL = clamp(dot(normal, light_direction), 0.0, 1.0);
    if NdotL <= 0.0 {
        return vec3<f32>(0.0);
    }
    let NdotV = max(dot(normal, view_direction), 0.0001);
    let half_direction = normalize(light_direction + view_direction);
    let NdotH = clamp(dot(normal, half_direction), 0.0, 1.0);
    let LdotH = clamp(dot(light_direction, half_direction), 0.0, 1.0);
    let roughness = clamp(perceptual_roughness * perceptual_roughness, 0.0001, 1.0);
    let f0 = mix(vec3<f32>(0.04), base_color, metallic);
    let fresnel_term = fresnel(f0, LdotH);
    let distribution = D_GGX(roughness, NdotH);
    let visibility = V_SmithGGXCorrelated(roughness, NdotV, NdotL);
    let specular = specular_multiscatter(
        distribution,
        visibility,
        fresnel_term,
        f0,
        moonly_f_ab(perceptual_roughness, NdotV),
        1.0,
    );
    let f90 = 0.5 + 2.0 * roughness * LdotH * LdotH;
    let diffuse = base_color * (1.0 - metallic)
        * F_Schlick(1.0, f90, NdotL)
        * F_Schlick(1.0, f90, NdotV)
        * MOONLY_INVERSE_PI;
    return (diffuse + specular) * NdotL;
}

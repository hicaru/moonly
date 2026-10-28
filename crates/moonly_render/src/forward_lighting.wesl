import package::lighting::moonly_brdf;
import bevy_pbr::render::pbr_types::PbrInput;

struct MoonlyForwardLight {
    position_range: vec4<f32>,
    direction_cos: vec4<f32>,
    color: vec4<f32>,
    shadow_from_world: mat4x4<f32>,
    shadow_settings: vec4<f32>,
    shadow_transform: vec4<f32>,
    misc: vec4<f32>,
};

struct MoonlyForwardParams {
    ambient: vec4<f32>,
    counts: vec4<u32>,
};

@group(4) @binding(0) var<storage, read> forward_lights: array<MoonlyForwardLight>;
@group(4) @binding(1) var<uniform> forward_params: MoonlyForwardParams;
@group(4) @binding(2) var forward_shadow_atlas: texture_depth_2d;
@group(4) @binding(3) var forward_shadow_sampler: sampler_comparison;
@group(4) @binding(4) var forward_sun_near: texture_depth_2d;
@group(4) @binding(5) var forward_sun_middle: texture_depth_2d;
@group(4) @binding(6) var forward_sun_far: texture_depth_2d;
@group(4) @binding(7) var forward_sun_sampler: sampler_comparison;
@group(4) @binding(8) var<uniform> forward_sun: MoonlySunUniform;

struct MoonlySunUniform {
    view_position: vec4<f32>,
    view_forward: vec4<f32>,
    direction_bias: vec4<f32>,
    color: vec4<f32>,
    splits_near: vec4<f32>,
    splits_far: vec4<f32>,
    cascade_biases: vec4<f32>,
    blend_bands: vec4<f32>,
    shadow_near_from_world: mat4x4<f32>,
    shadow_middle_from_world: mat4x4<f32>,
    shadow_far_from_world: mat4x4<f32>,
    previous_near_from_world: mat4x4<f32>,
    previous_middle_from_world: mat4x4<f32>,
    previous_far_from_world: mat4x4<f32>,
};

fn forward_attenuation(light: MoonlyForwardLight, distance_to_light: f32) -> f32 {
    if light.shadow_settings.w > 1.5 {
        return 1.0;
    }
    if light.misc.y < 0.5 {
        let attenuation = clamp(1.0 - distance_to_light / light.position_range.w, 0.0, 1.0);
        return attenuation * attenuation;
    }
    let distance_squared = distance_to_light * distance_to_light;
    let inverse_range_squared = 1.0 / max(light.position_range.w * light.position_range.w, 0.0001);
    let factor = distance_squared * inverse_range_squared;
    let smooth_factor = clamp(1.0 - factor * factor, 0.0, 1.0);
    return smooth_factor * smooth_factor / max(distance_squared, 0.0001);
}

fn forward_cone(light: MoonlyForwardLight, light_direction: vec3<f32>) -> f32 {
    if light.shadow_settings.w < 0.5 || light.shadow_settings.w > 1.5 {
        return 1.0;
    }
    let cone_cos = dot(-light_direction, normalize(light.direction_cos.xyz));
    let reference_cone = smoothstep(
        light.direction_cos.w,
        min(light.direction_cos.w + 0.15, 1.0),
        cone_cos,
    );
    let cone_scale = 1.0 / max(light.misc.x - light.direction_cos.w, 0.0001);
    let native_attenuation = clamp(
        cone_cos * cone_scale - light.direction_cos.w * cone_scale,
        0.0,
        1.0,
    );
    let native_cone = native_attenuation * native_attenuation;
    return select(reference_cone, native_cone, light.misc.x >= 0.0);
}

fn forward_cascade_shadow(
    atlas: texture_depth_2d,
    from_world: mat4x4<f32>,
    world_position: vec3<f32>,
) -> f32 {
    let clip = from_world * vec4(world_position, 1.0);
    if clip.w <= 0.0 {
        return 0.0;
    }
    let ndc = clip.xyz / clip.w;
    let uv = ndc.xy * vec2(0.5, -0.5) + vec2(0.5);
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) {
        return 1.0;
    }
    var visibility = 0.0;
    for (var tap = 0u; tap < 4u; tap += 1u) {
        let offset = vec2(
            select(-0.5, 0.5, (tap & 1u) != 0u),
            select(-0.5, 0.5, (tap & 2u) != 0u),
        );
        visibility += textureSampleCompareLevel(
            atlas,
            forward_sun_sampler,
            clamp(uv + offset * forward_sun.view_position.w, vec2(0.0), vec2(1.0)),
            ndc.z + forward_sun.direction_bias.w,
        );
    }
    return visibility * 0.25;
}

fn forward_sun_shadow(world_position: vec3<f32>) -> f32 {
    if forward_params.counts.y == 0u {
        return 1.0;
    }
    let view_depth = dot(
        world_position - forward_sun.view_position.xyz,
        forward_sun.view_forward.xyz,
    );
    if view_depth >= forward_sun.splits_far.z {
        return 1.0;
    }
    if view_depth < forward_sun.splits_far.x {
        return forward_cascade_shadow(
            forward_sun_near,
            forward_sun.shadow_near_from_world,
            world_position,
        );
    }
    if view_depth < forward_sun.splits_far.y {
        return forward_cascade_shadow(
            forward_sun_middle,
            forward_sun.shadow_middle_from_world,
            world_position,
        );
    }
    return forward_cascade_shadow(
        forward_sun_far,
        forward_sun.shadow_far_from_world,
        world_position,
    );
}

fn forward_shadow(light: MoonlyForwardLight, world_position: vec3<f32>) -> f32 {
    if light.shadow_settings.w > 1.5 {
        return forward_sun_shadow(world_position);
    }
    if light.shadow_settings.x < 0.5 {
        return 1.0;
    }
    let clip = light.shadow_from_world * vec4(world_position, 1.0);
    if clip.w <= 0.0 {
        return 0.0;
    }
    let ndc = clip.xyz / clip.w;
    let local_uv = ndc.xy * vec2(0.5, -0.5) + vec2(0.5);
    if any(local_uv < vec2(0.0)) || any(local_uv > vec2(1.0)) {
        return 1.0;
    }
    let receiver_depth = ndc.z - light.shadow_settings.y;
    var visibility = 0.0;
    for (var tap = 0u; tap < 4u; tap += 1u) {
        let offset = vec2(
            select(-0.5, 0.5, (tap & 1u) != 0u),
            select(-0.5, 0.5, (tap & 2u) != 0u),
        );
        let sample_uv = light.shadow_transform.xy
            + (local_uv + offset * light.shadow_settings.z) * light.shadow_transform.z;
        visibility += textureSampleCompareLevel(
            forward_shadow_atlas,
            forward_shadow_sampler,
            sample_uv,
            receiver_depth,
        );
    }
    return visibility * 0.25;
}

fn apply_moonly_forward_lighting(pbr_input: PbrInput) -> vec4<f32> {
    let material = pbr_input.material;
    var radiance = forward_params.ambient.rgb * material.base_color.rgb
        * (1.0 - material.metallic) + material.emissive.rgb;
    for (var index = 0u; index < forward_params.counts.x; index += 1u) {
        let light = forward_lights[index];
        var light_direction = -normalize(light.direction_cos.xyz);
        var distance_to_light = 0.0;
        if light.shadow_settings.w < 1.5 {
            let to_light = light.position_range.xyz - pbr_input.world_position.xyz;
            distance_to_light = length(to_light);
            if distance_to_light >= light.position_range.w {
                continue;
            }
            light_direction = to_light / max(distance_to_light, 0.0001);
        }
        let cone = forward_cone(light, light_direction);
        if cone <= 0.0 {
            continue;
        }
        let attenuation = forward_attenuation(light, distance_to_light);
        let visibility = forward_shadow(light, pbr_input.world_position.xyz);
        let surface = moonly_brdf(
            normalize(pbr_input.N),
            normalize(pbr_input.V),
            light_direction,
            material.base_color.rgb,
            material.metallic,
            material.perceptual_roughness,
        );
        radiance += light.color.rgb * surface * (cone * attenuation * visibility);
    }
    return vec4(radiance, material.base_color.a);
}

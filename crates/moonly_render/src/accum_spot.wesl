import package::lighting::{moonly_brdf, moonly_legacy_brdf};

@group(0) @binding(0) var position_texture: texture_2d<f32>;
@group(0) @binding(1) var normal_texture: texture_2d<f32>;
@group(0) @binding(2) var albedo_gloss_texture: texture_2d<f32>;
@group(0) @binding(3) var<uniform> spot: MoonlySpotUniform;
@group(0) @binding(6) var shadow_atlas: texture_depth_2d;
@group(0) @binding(7) var shadow_sampler: sampler_comparison;
@group(0) @binding(8) var projector_texture: texture_2d<f32>;
@group(0) @binding(9) var projector_sampler: sampler;
@group(0) @binding(10) var jitter_texture: texture_2d_array<f32>;
@group(0) @binding(11) var jitter_sampler: sampler;
@group(0) @binding(12) var transmittance_atlas: texture_2d<f32>;

struct MoonlySpotUniform {
    position_range: vec4<f32>,
    direction_cos: vec4<f32>,
    color: vec4<f32>,
    view_position: vec4<f32>,
    shadow_from_world: mat4x4<f32>,
    projector_from_world: mat4x4<f32>,
    clip_from_local: mat4x4<f32>,
    shadow_settings: vec4<f32>,
    shadow_transform: vec4<f32>,
    world_from_clip: mat4x4<f32>,
    volumetric: vec4<f32>,
    screen_attenuation: vec4<f32>,
    volumetric_schedule: vec4<f32>,
};

struct SpotVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) clip_position: vec4<f32>,
};

@vertex
fn vertex(@location(0) local_position: vec3<f32>) -> SpotVertexOutput {
    var out: SpotVertexOutput;
    out.position = spot.clip_from_local * vec4(local_position, 1.0);
    out.clip_position = out.position;
    return out;
}

fn light_attenuation(light_to_fragment: vec3<f32>) -> f32 {
    let distance_squared = dot(light_to_fragment, light_to_fragment);
    let linear = clamp(
        1.0 - distance_squared * spot.screen_attenuation.z,
        0.0,
        1.0,
    );
    let reference = linear * linear;
    let inverse_range_squared = 1.0 / max(
        spot.position_range.w * spot.position_range.w,
        0.0001,
    );
    let factor = distance_squared * inverse_range_squared;
    let smooth_factor = clamp(1.0 - factor * factor, 0.0, 1.0);
    let native = smooth_factor * smooth_factor / max(distance_squared, 0.0001);
    return select(reference, native, spot.color.a >= 0.5);
}

@fragment
fn mask_fragment() -> @location(0) vec4<f32> {
    return vec4(0.0);
}

@fragment
fn fragment(in: SpotVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    let position = textureLoad(position_texture, pixel, 0);
    if position.w < 0.5 {
        return vec4(0.0);
    }
    let to_light = spot.position_range.xyz - position.xyz;
    let distance_to_light = length(to_light);
    if distance_to_light >= spot.position_range.w {
        return vec4(0.0);
    }
    let light_direction = to_light / max(distance_to_light, 0.0001);
    let cone_cos = dot(-light_direction, normalize(spot.direction_cos.xyz));
    let reference_cone = smoothstep(
        spot.direction_cos.w,
        min(spot.direction_cos.w + 0.15, 1.0),
        cone_cos,
    );
    let cone_scale = 1.0 / max(spot.volumetric.w - spot.direction_cos.w, 0.0001);
    let native_attenuation = clamp(
        cone_cos * cone_scale - spot.direction_cos.w * cone_scale,
        0.0,
        1.0,
    );
    let native_cone = native_attenuation * native_attenuation;
    let spot_cone = select(reference_cone, native_cone, spot.volumetric.w >= 0.0);
    let cone = select(spot_cone, 1.0, spot.direction_cos.w < -0.999);
    if cone <= 0.0 {
        return vec4(0.0);
    }

    let normal_sample = textureLoad(normal_texture, pixel, 0);
    let normal = normalize(normal_sample.xyz);
    let albedo_gloss = textureLoad(albedo_gloss_texture, pixel, 0);
    let view_direction = normalize(spot.view_position.xyz - position.xyz);
    var surface = moonly_legacy_brdf(
        normal,
        view_direction,
        light_direction,
        albedo_gloss,
        normal_sample.w,
    );
    if spot.color.a >= 0.5 {
        surface = moonly_brdf(
            normal,
            view_direction,
            light_direction,
            albedo_gloss.rgb,
            max(position.w - 1.0, 0.0),
            1.0 - albedo_gloss.a,
        );
    }
    let projection = projector_color(position.xyz);
    let radiance = spot.color.rgb * projection * (cone * light_attenuation(to_light));
    let visibility = shadow_visibility(position.xyz, in.position.xy);
    return vec4(radiance * surface * visibility, 1.0);
}

fn shadow_visibility(world_position: vec3<f32>, screen_xy: vec2<f32>) -> vec3<f32> {
    if spot.shadow_settings.x < 0.5 {
        return vec3(1.0);
    }
    let clip = spot.shadow_from_world * vec4(world_position, 1.0);
    if clip.w <= 0.0 {
        return vec3(0.0);
    }
    let ndc = clip.xyz / clip.w;
    let local_uv = ndc.xy * vec2(0.5, -0.5) + vec2(0.5);
    let receiver_depth = ndc.z - spot.shadow_settings.y;
    let jitter_uv = screen_xy / 64.0;
    var visibility = 0.0;
    for (var layer = 0u; layer < 4u; layer += 1u) {
        let jitter = textureSampleLevel(
            jitter_texture,
            jitter_sampler,
            jitter_uv,
            i32(layer),
            0.0,
        ).xy;
        let sample_local = clamp(
            local_uv + jitter * spot.shadow_settings.z,
            vec2(0.0),
            vec2(1.0),
        );
        let sample_uv = spot.shadow_transform.xy + sample_local * spot.shadow_transform.z;
        visibility += textureSampleCompareLevel(
            shadow_atlas,
            shadow_sampler,
            sample_uv,
            receiver_depth,
        );
    }
    let transmittance_uv = spot.shadow_transform.xy + local_uv * spot.shadow_transform.z;
    let transmittance = textureSampleLevel(
        transmittance_atlas,
        projector_sampler,
        transmittance_uv,
        0.0,
    ).rgb;
    return mix(transmittance, vec3(1.0), visibility * 0.25);
}

fn projector_color(world_position: vec3<f32>) -> vec3<f32> {
    if spot.shadow_settings.w < 0.5 {
        return vec3(1.0);
    }
    let clip = spot.projector_from_world * vec4(world_position, 1.0);
    if clip.w <= 0.0 {
        return vec3(0.0);
    }
    let ndc = clip.xy / clip.w;
    let uv = ndc * vec2(0.5, -0.5) + vec2(0.5);
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) {
        return vec3(0.0);
    }
    return textureSampleLevel(projector_texture, projector_sampler, uv, 0.0).rgb;
}

fn volumetric_attenuation(light_to_fragment: vec3<f32>) -> f32 {
    let distance_squared = dot(light_to_fragment, light_to_fragment);
    let distance_scale = max(spot.volumetric.y * spot.volumetric.y, 0.0001);
    let linear = clamp(
        1.0 - distance_squared * spot.screen_attenuation.z / distance_scale,
        0.0,
        1.0,
    );
    return linear * linear;
}

fn local_scattering(world_position: vec3<f32>, screen_xy: vec2<f32>) -> vec3<f32> {
    let to_light = spot.position_range.xyz - world_position;
    let distance = length(to_light);
    if distance >= spot.position_range.w || distance < 0.0001 {
        return vec3(0.0);
    }
    let direction = to_light / distance;
    let cone_cos = dot(-direction, normalize(spot.direction_cos.xyz));
    let cone = select(
        smoothstep(spot.direction_cos.w, min(spot.direction_cos.w + 0.15, 1.0), cone_cos),
        1.0,
        spot.direction_cos.w < -0.999,
    );
    let visibility = shadow_visibility(world_position, screen_xy);
    return spot.color.rgb * projector_color(world_position)
        * cone * volumetric_attenuation(to_light) * visibility;
}

@fragment
fn volumetric_fragment(in: SpotVertexOutput) -> @location(0) vec4<f32> {
    if spot.volumetric.x <= 0.0 {
        return vec4(0.0);
    }
    let ndc = in.clip_position.xy / in.clip_position.w;
    let uv = ndc * vec2(0.5, -0.5) + vec2(0.5);
    let screen_xy = uv * spot.screen_attenuation.xy;
    let dimensions = vec2<f32>(textureDimensions(position_texture));
    let position = textureLoad(position_texture, vec2<i32>(uv * dimensions), 0);
    let far = spot.world_from_clip * vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 1.0, 1.0);
    var end_position = far.xyz / far.w;
    if position.w > 0.5 {
        end_position = position.xyz;
    }
    let ray = end_position - spot.view_position.xyz;
    let ray_length = length(ray);
    let max_march = spot.volumetric_schedule.x * spot.volumetric_schedule.z;
    let march_length = min(ray_length, max_march) * spot.volumetric.y;
    if march_length < 0.001 {
        return vec4(0.0);
    }
    let direction = ray / max(ray_length, 0.001);
    let steps = i32(spot.volumetric_schedule.z);
    let step_length = march_length * spot.volumetric_schedule.y;
    var scattering = vec3(0.0);
    for (var index = 0; index < steps; index += 1) {
        let distance = (f32(index) + 0.5) * step_length;
        let point = spot.view_position.xyz + direction * distance;
        scattering += local_scattering(point, screen_xy) * step_length;
    }
    return vec4(scattering * spot.volumetric.x, 1.0);
}

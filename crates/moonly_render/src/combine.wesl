@group(0) @binding(0) var position_texture: texture_2d<f32>;
@group(0) @binding(1) var normal_texture: texture_2d<f32>;
@group(0) @binding(2) var albedo_gloss_texture: texture_2d<f32>;
@group(0) @binding(3) var accumulator_texture: texture_2d<f32>;
@group(0) @binding(4) var depth_texture: texture_2d<f32>;
@group(0) @binding(5) var shadow_atlas: texture_depth_2d;
@group(0) @binding(6) var<uniform> environment: MoonlyEnvironmentUniform;
@group(0) @binding(7) var ao_texture: texture_2d<f32>;
@group(0) @binding(8) var volumetric_texture: texture_2d<f32>;
@group(0) @binding(9) var local_volumetric_texture: texture_2d<f32>;
@group(0) @binding(10) var froxel_texture: texture_2d<f32>;
@group(0) @binding(11) var emissive_texture: texture_2d<f32>;

struct MoonlyEnvironmentUniform {
    ambient: vec4<f32>,
    hemi: vec4<f32>,
    fog_color: vec4<f32>,
    fog_plane: vec4<f32>,
    ao: vec4<f32>,
    view_position: vec4<f32>,
    fog_params: vec4<f32>,
};

const MOONLY_HEMI_SCALE: f32 = 0.75;

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn colored_ao(ao: f32, albedo: vec3<f32>) -> vec3<f32> {
    let a = 2.0404 * albedo - vec3(0.3324);
    let b = -4.7951 * albedo + vec3(0.6417);
    let c = 2.7552 * albedo + vec3(0.6903);
    let polynomial = ((vec3(ao) * a + b) * ao + c) * ao;
    return max(vec3(ao), polynomial);
}

fn local_volume(uv: vec2<f32>, world_position: vec3<f32>) -> vec3<f32> {
    let low_size = vec2<f32>(textureDimensions(local_volumetric_texture));
    let full_size = vec2<f32>(textureDimensions(position_texture));
    let sample_position = uv * low_size - vec2(0.5);
    let base = floor(sample_position);
    let fraction = sample_position - base;
    var color = vec3(0.0);
    var weight_sum = 0.0;
    for (var y = 0; y < 2; y += 1) {
        for (var x = 0; x < 2; x += 1) {
            let offset = vec2(f32(x), f32(y));
            let low_pixel = vec2<i32>(base + offset);
            let low_uv = (base + offset + vec2(0.5)) / low_size;
            let surface = textureLoad(position_texture, vec2<i32>(low_uv * full_size), 0);
            let depth_weight = 1.0 / (length(surface.xyz - world_position) + 0.05);
            let bilinear = select(1.0 - fraction.x, fraction.x, x == 1)
                * select(1.0 - fraction.y, fraction.y, y == 1);
            let weight = depth_weight * bilinear;
            color += textureLoad(local_volumetric_texture, low_pixel, 0).rgb * weight;
            weight_sum += weight;
        }
    }
    return color / max(weight_sum, 0.0001);
}

fn apply_froxel(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let dimensions = vec2<f32>(textureDimensions(froxel_texture));
    let sample = textureLoad(froxel_texture, vec2<i32>(uv * dimensions), 0);
    return color * sample.a + sample.rgb;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(position_texture));
    let pixel = vec2<i32>(in.uv * dimensions);
    let position = textureLoad(position_texture, pixel, 0);
    if position.w < 0.5 {
        if constants::MOONLY_OVERLAY == 1u {
            return vec4(0.0);
        }
        let local = textureLoad(
            local_volumetric_texture,
            vec2<i32>(in.uv * vec2<f32>(textureDimensions(local_volumetric_texture))),
            0,
        ).rgb;
        let color = textureLoad(volumetric_texture, pixel, 0).rgb + local;
        return vec4(apply_froxel(color, in.uv), 1.0);
    }
    if constants::MOONLY_DEBUG_VIEW == 1u {
        return vec4(position.xyz, 1.0);
    }
    if constants::MOONLY_DEBUG_VIEW == 3u {
        return vec4(textureLoad(albedo_gloss_texture, pixel, 0).rgb, 1.0);
    }
    if constants::MOONLY_DEBUG_VIEW == 4u {
        let depth = textureLoad(depth_texture, pixel, 0).x;
        return vec4(depth, depth, depth, 1.0);
    }
    if constants::MOONLY_DEBUG_VIEW == 5u {
        return vec4(textureLoad(accumulator_texture, pixel, 0).rgb, 1.0);
    }
    if constants::MOONLY_DEBUG_VIEW == 6u {
        let depth = textureLoad(shadow_atlas, pixel, 0);
        return vec4(depth, depth, depth, 1.0);
    }
    if constants::MOONLY_DEBUG_VIEW == 7u {
        return vec4(textureLoad(ao_texture, pixel, 0).rgb, 1.0);
    }
    if constants::MOONLY_DEBUG_VIEW == 8u {
        let sun = textureLoad(volumetric_texture, pixel, 0).rgb;
        return vec4((sun + local_volume(in.uv, position.xyz)) * 32.0, 1.0);
    }
    if constants::MOONLY_DEBUG_VIEW > 8u {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }

    let normal_sample = textureLoad(normal_texture, pixel, 0);
    let normal = normalize(normal_sample.xyz);
    let unlit = normal_sample.w < 0.0;
    if constants::MOONLY_DEBUG_VIEW == 2u {
        return vec4(normal * 0.5 + vec3(0.5), 1.0);
    }
    let albedo_gloss = textureLoad(albedo_gloss_texture, pixel, 0);
    let accumulator = textureLoad(accumulator_texture, pixel, 0);
    let emissive = textureLoad(emissive_texture, pixel, 0);
    let separate_emissive = f32(environment.fog_params.w > 0.5);
    let accumulated = accumulator.rgb - emissive.rgb * separate_emissive;
    let scalar_ao = textureLoad(ao_texture, pixel, 0).r;
    let ao = select(vec3(scalar_ao), colored_ao(scalar_ao, albedo_gloss.rgb), environment.ao.x >= 2.5);
    let ambient = environment.ambient.rgb + environment.hemi.rgb * environment.hemi.w * MOONLY_HEMI_SCALE;
    let global_light = albedo_gloss.rgb * ambient * ao + accumulated;
    let tracked_light = accumulated * ao;
    let tracked_only = accumulator.w > 1.5 && environment.fog_params.w < 0.5;
    let direct_color = select(select(global_light, tracked_light, tracked_only), accumulator.rgb, unlit)
        + textureLoad(volumetric_texture, pixel, 0).rgb
        + local_volume(in.uv, position.xyz);
    let exposure = select(1.0, environment.ambient.a, environment.fog_params.w > 0.5);
    let emissive_exposure = select(1.0, mix(1.0, exposure, emissive.a), environment.fog_params.w > 0.5);
    let color = apply_froxel(direct_color, in.uv) * exposure
        + emissive.rgb * emissive_exposure * separate_emissive;
    let distance_fog = clamp(dot(vec4(position.xyz, 1.0), environment.fog_plane), 0.0, 1.0);
    let height = exp(-environment.fog_params.y * (position.y - environment.fog_params.z));
    let optical_depth = environment.fog_params.x * distance(position.xyz, environment.view_position.xyz) * height;
    let density_fog = 1.0 - exp(-optical_depth);
    let fog = max(distance_fog, density_fog);
    return vec4(mix(color, environment.fog_color.rgb, fog), 1.0);
}

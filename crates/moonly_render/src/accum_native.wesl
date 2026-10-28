import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;
import package::lighting::moonly_brdf;

@group(0) @binding(0) var position_texture: texture_2d<f32>;
@group(0) @binding(1) var normal_texture: texture_2d<f32>;
@group(0) @binding(2) var albedo_gloss_texture: texture_2d<f32>;
@group(0) @binding(3) var<storage, read> lights: array<MoonlySpotUniform>;
@group(0) @binding(6) var shadow_atlas: texture_depth_2d;
@group(0) @binding(7) var shadow_sampler: sampler_comparison;
@group(0) @binding(8) var projector_texture: texture_2d<f32>;
@group(0) @binding(9) var projector_sampler: sampler;
@group(0) @binding(10) var jitter_texture: texture_2d_array<f32>;
@group(0) @binding(11) var jitter_sampler: sampler;
@group(0) @binding(12) var<uniform> params: MoonlyNativeLocalParams;
@group(0) @binding(13) var<storage, read> tiles: array<NativeTile>;
@group(0) @binding(14) var<storage, read> indices: array<u32>;

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
    screen_size: vec4<f32>,
};

struct MoonlyNativeLocalParams {
    grid: vec4<u32>,
};

struct NativeTile {
    offset: u32,
    count: u32,
};

struct MoonlyNativeSurface {
    position: vec4<f32>,
    normal: vec3<f32>,
    albedo_gloss: vec4<f32>,
    view_direction: vec3<f32>,
};

@vertex
fn fullscreen_vertex_shader(@builtin(vertex_index) index: u32) -> FullscreenVertexOutput {
    let uv = vec2<f32>(vec2<u32>((index << 1u) & 2u, index & 2u));
    var out: FullscreenVertexOutput;
    out.position = vec4(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2(uv.x, 1.0 - uv.y);
    return out;
}

fn attenuation(range: f32, distance: f32) -> f32 {
    let distance_squared = distance * distance;
    let inverse_range_squared = 1.0 / max(range * range, 0.0001);
    let factor = distance_squared * inverse_range_squared;
    let smooth_factor = clamp(1.0 - factor * factor, 0.0, 1.0);
    return smooth_factor * smooth_factor / max(distance_squared, 0.0001);
}

fn cone_factor(direction_cos: vec4<f32>, inner_cos: f32, direction: vec3<f32>) -> f32 {
    let cone_cos = dot(-direction, normalize(direction_cos.xyz));
    let reference = smoothstep(direction_cos.w, min(direction_cos.w + 0.15, 1.0), cone_cos);
    let scale = 1.0 / max(inner_cos - direction_cos.w, 0.0001);
    let native_attenuation = clamp(cone_cos * scale - direction_cos.w * scale, 0.0, 1.0);
    let native_cone = native_attenuation * native_attenuation;
    return select(select(reference, native_cone, inner_cos >= 0.0), 1.0, direction_cos.w < -0.999);
}

fn projector_color(index: u32, world_position: vec3<f32>) -> vec3<f32> {
    if lights[index].shadow_settings.w < 0.5 {
        return vec3(1.0);
    }
    let clip = lights[index].projector_from_world * vec4(world_position, 1.0);
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

fn shadow_visibility(index: u32, world_position: vec3<f32>, screen_xy: vec2<f32>) -> f32 {
    let settings = lights[index].shadow_settings;
    if settings.x < 0.5 {
        return 1.0;
    }
    let clip = lights[index].shadow_from_world * vec4(world_position, 1.0);
    if clip.w <= 0.0 {
        return 0.0;
    }
    let ndc = clip.xyz / clip.w;
    let local_uv = ndc.xy * vec2(0.5, -0.5) + vec2(0.5);
    let receiver_depth = ndc.z - settings.y;
    let jitter_uv = screen_xy / 64.0;
    let rect = lights[index].shadow_transform;
    var visibility = 0.0;
    for (var layer = 0u; layer < 4u; layer += 1u) {
        let jitter = textureSampleLevel(jitter_texture, jitter_sampler, jitter_uv, i32(layer), 0.0).xy;
        let sample_local = clamp(local_uv + jitter * settings.z, vec2(0.0), vec2(1.0));
        let sample_uv = rect.xy + sample_local * rect.z;
        visibility += textureSampleCompareLevel(shadow_atlas, shadow_sampler, sample_uv, receiver_depth);
    }
    return visibility * 0.25;
}

fn contribution(index: u32, surface: MoonlyNativeSurface, screen_xy: vec2<f32>) -> vec3<f32> {
    let position_range = lights[index].position_range;
    let to_light = position_range.xyz - surface.position.xyz;
    let distance = length(to_light);
    if distance >= position_range.w {
        return vec3(0.0);
    }
    let direction = to_light / max(distance, 0.0001);
    let direction_cos = lights[index].direction_cos;
    let cone = cone_factor(direction_cos, lights[index].volumetric.w, direction);
    if cone <= 0.0 || dot(surface.normal, direction) <= 0.0 {
        return vec3(0.0);
    }
    let brdf = moonly_brdf(
        surface.normal,
        surface.view_direction,
        direction,
        surface.albedo_gloss.rgb,
        max(surface.position.w - 1.0, 0.0),
        1.0 - surface.albedo_gloss.a,
    );
    let radiance = lights[index].color.rgb * projector_color(index, surface.position.xyz)
        * cone * attenuation(position_range.w, distance);
    return radiance * brdf * shadow_visibility(index, surface.position.xyz, screen_xy);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(position_texture));
    let pixel = vec2<i32>(in.uv * dimensions);
    let position = textureLoad(position_texture, pixel, 0);
    if position.w < 0.5 {
        return vec4(0.0);
    }
    let normal_sample = textureLoad(normal_texture, pixel, 0);
    if normal_sample.w < 0.0 {
        return vec4(0.0);
    }
    var surface: MoonlyNativeSurface;
    surface.position = position;
    surface.normal = normalize(normal_sample.xyz);
    surface.albedo_gloss = textureLoad(albedo_gloss_texture, pixel, 0);
    surface.view_direction = normalize(lights[0].view_position.xyz - position.xyz);
    let tile_xy = vec2<u32>(in.position.xy / 64.0);
    let tile_index = tile_xy.y * params.grid.x + tile_xy.x;
    let tile = tiles[tile_index];
    var radiance = vec3(0.0);
    for (var index = 0u; index < tile.count; index += 1u) {
        let light_index = indices[tile.offset + index];
        radiance += contribution(light_index, surface, in.position.xy);
    }
    return vec4(radiance, f32(tile.count));
}

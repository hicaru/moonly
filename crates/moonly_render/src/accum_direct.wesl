import package::lighting::{moonly_brdf, moonly_legacy_brdf};

@group(0) @binding(0) var position_texture: texture_2d<f32>;
@group(0) @binding(1) var normal_texture: texture_2d<f32>;
@group(0) @binding(2) var albedo_gloss_texture: texture_2d<f32>;
@group(0) @binding(3) var<uniform> sun: MoonlySunUniform;
@group(0) @binding(6) var shadow_near: texture_depth_2d;
@group(0) @binding(7) var shadow_middle: texture_depth_2d;
@group(0) @binding(8) var shadow_far: texture_depth_2d;
@group(0) @binding(9) var shadow_sampler: sampler_comparison;
@group(0) @binding(10) var near_minmax: texture_2d<f32>;
@group(0) @binding(11) var jitter_texture: texture_2d_array<f32>;
@group(0) @binding(12) var jitter_sampler: sampler;

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

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(position_texture));
    let pixel = vec2<i32>(in.uv * dimensions);
    let position = textureLoad(position_texture, pixel, 0);
    if position.w < 0.5 {
        return vec4(0.0);
    }
    let normal_sample = textureLoad(normal_texture, pixel, 0);
    let normal = normalize(normal_sample.xyz);
    let light_direction = normalize(-sun.direction_bias.xyz);
    let view_direction = normalize(sun.view_position.xyz - position.xyz);
    let albedo_gloss = textureLoad(albedo_gloss_texture, pixel, 0);
    let view_depth = dot(position.xyz - sun.view_position.xyz, sun.view_forward.xyz);
    if view_depth >= sun.splits_far.z {
        return vec4(0.0);
    }
    var visibility = 0.0;
    if view_depth < sun.splits_far.x {
        let current = cascade_visibility(
            shadow_near,
            sun.shadow_near_from_world,
            position.xyz,
            sun.splits_far.w > 0.5,
            in.position.xy,
            sun.cascade_biases.x,
        );
        let previous = cascade_visibility(
            shadow_near,
            sun.previous_near_from_world,
            position.xyz,
            sun.splits_far.w > 0.5,
            in.position.xy,
            sun.cascade_biases.x,
        );
        visibility = mix(
            previous,
            current,
            transition_weight(view_depth, sun.splits_near.x, sun.blend_bands.x),
        );
    } else if view_depth < sun.splits_far.y {
        let current = cascade_visibility(
            shadow_middle,
            sun.shadow_middle_from_world,
            position.xyz,
            false,
            in.position.xy,
            sun.cascade_biases.y,
        );
        let previous = cascade_visibility(
            shadow_near,
            sun.previous_middle_from_world,
            position.xyz,
            false,
            in.position.xy,
            sun.cascade_biases.x,
        );
        visibility = mix(
            previous,
            current,
            transition_weight(view_depth, sun.splits_near.y, sun.blend_bands.y),
        );
    } else {
        let current = cascade_visibility(
            shadow_far,
            sun.shadow_far_from_world,
            position.xyz,
            false,
            in.position.xy,
            sun.cascade_biases.z,
        );
        let previous = cascade_visibility(
            shadow_middle,
            sun.previous_far_from_world,
            position.xyz,
            false,
            in.position.xy,
            sun.cascade_biases.y,
        );
        visibility = mix(
            previous,
            current,
            transition_weight(view_depth, sun.splits_near.z, sun.blend_bands.z),
        );
    }
    var surface = moonly_legacy_brdf(
        normal,
        view_direction,
        light_direction,
        albedo_gloss,
        normal_sample.w,
    );
    if sun.color.a >= 0.5 {
        surface = moonly_brdf(
            normal,
            view_direction,
            light_direction,
            albedo_gloss.rgb,
            max(position.w - 1.0, 0.0),
            1.0 - albedo_gloss.a,
        );
    }
    return vec4(sun.color.rgb * surface * visibility, 1.0);
}

fn transition_weight(view_depth: f32, split_near: f32, blend_band: f32) -> f32 {
    if blend_band <= 0.0 {
        return select(0.0, 1.0, view_depth >= split_near);
    }
    return clamp((view_depth - split_near) / blend_band, 0.0, 1.0);
}

fn cascade_visibility(
    shadow_texture: texture_depth_2d,
    shadow_from_world: mat4x4<f32>,
    world_position: vec3<f32>,
    use_minmax: bool,
    screen_xy: vec2<f32>,
    depth_bias: f32,
) -> f32 {
    let clip = shadow_from_world * vec4(world_position, 1.0);
    if clip.w <= 0.0 {
        return 0.0;
    }
    let ndc = clip.xyz / clip.w;
    let uv = ndc.xy * vec2(0.5, -0.5) + vec2(0.5);
    if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) {
        return 1.0;
    }
    let receiver_depth = ndc.z + depth_bias;
    if use_minmax {
        let dimensions = vec2<f32>(textureDimensions(near_minmax));
        let limits = textureLoad(near_minmax, vec2<i32>(uv * dimensions), 0).rg;
        if receiver_depth < limits.x {
            return 1.0;
        }
        if receiver_depth > limits.y {
            return 0.0;
        }
    }
    var visibility = 0.0;
    for (var layer = 0u; layer < 4u; layer += 1u) {
        let jitter = textureSampleLevel(
            jitter_texture,
            jitter_sampler,
            screen_xy / 64.0,
            i32(layer),
            0.0,
        ).xy;
        visibility += textureSampleCompareLevel(
            shadow_texture,
            shadow_sampler,
            clamp(uv + jitter * sun.view_position.w, vec2(0.0), vec2(1.0)),
            receiver_depth,
        );
    }
    return visibility * 0.25;
}

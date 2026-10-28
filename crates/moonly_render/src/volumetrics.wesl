@group(0) @binding(0) var position_texture: texture_2d<f32>;
@group(0) @binding(1) var<uniform> volume: MoonlyVolumetricUniform;
@group(0) @binding(2) var shadow_near: texture_depth_2d;
@group(0) @binding(3) var shadow_middle: texture_depth_2d;
@group(0) @binding(4) var shadow_far: texture_depth_2d;
@group(0) @binding(5) var shadow_sampler: sampler_comparison;

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

struct MoonlyVolumetricUniform {
    sun: MoonlySunUniform,
    world_from_clip: mat4x4<f32>,
    params: vec4<f32>,
    screen: vec4<f32>,
};

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn cascade_visibility(
    shadow_texture: texture_depth_2d,
    shadow_from_world: mat4x4<f32>,
    world_position: vec3<f32>,
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
    return textureSampleCompareLevel(
        shadow_texture,
        shadow_sampler,
        uv,
        ndc.z + volume.sun.direction_bias.w,
    );
}

fn visibility(world_position: vec3<f32>, view_depth: f32) -> f32 {
    if view_depth < volume.sun.splits_far.x {
        return cascade_visibility(shadow_near, volume.sun.shadow_near_from_world, world_position);
    }
    if view_depth < volume.sun.splits_far.y {
        return cascade_visibility(shadow_middle, volume.sun.shadow_middle_from_world, world_position);
    }
    if view_depth < volume.sun.splits_far.z {
        return cascade_visibility(shadow_far, volume.sun.shadow_far_from_world, world_position);
    }
    return 0.0;
}

fn mie_phase(cos_theta: f32, g: f32) -> f32 {
    let g2 = g * g;
    let denominator = pow(max(1.0 + g2 - 2.0 * g * cos_theta, 0.0001), 1.5);
    return (1.0 - g2) / (4.0 * 3.14159265 * denominator);
}

fn raymarched_shafts(uv: vec2<f32>, position: vec4<f32>) -> vec3<f32> {
    let ndc = vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 1.0, 1.0);
    let far = volume.world_from_clip * ndc;
    var end_position = far.xyz / far.w;
    if position.w > 0.5 {
        end_position = position.xyz;
    }
    let ray = end_position - volume.sun.view_position.xyz;
    let ray_length = min(length(ray), 80.0);
    if ray_length < 0.001 {
        return vec3(0.0);
    }
    let direction = ray / max(length(ray), 0.001);
    let steps = i32(clamp(volume.params.y, 4.0, 64.0));
    let step_length = ray_length / f32(steps);
    let phase = mie_phase(dot(direction, -volume.sun.direction_bias.xyz), volume.params.z);
    var scattering = 0.0;
    for (var index = 0; index < steps; index += 1) {
        let distance = (f32(index) + 0.5) * step_length;
        let point = volume.sun.view_position.xyz + direction * distance;
        let height = exp(-0.08 * max(point.y, 0.0));
        scattering += visibility(point, distance) * height * step_length;
    }
    return volume.sun.color.rgb * scattering * phase * volume.params.x * 0.05;
}

fn screen_space_shafts(uv: vec2<f32>) -> vec3<f32> {
    if volume.screen.w < 0.5 {
        return vec3(0.0);
    }
    let steps = i32(clamp(volume.params.y, 4.0, 64.0));
    let delta = (volume.screen.xy - uv) / f32(steps);
    var sample_uv = uv;
    var weight = 1.0;
    var illumination = 0.0;
    for (var index = 0; index < steps; index += 1) {
        sample_uv += delta;
        let bounded = clamp(sample_uv, vec2(0.0), vec2(0.999999));
        let dimensions = vec2<f32>(textureDimensions(position_texture));
        let position = textureLoad(position_texture, vec2<i32>(bounded * dimensions), 0);
        let source = select(0.0, 1.0, position.w < 0.5);
        let sun_disc = 1.0 - smoothstep(0.015, 0.09, distance(sample_uv, volume.screen.xy));
        illumination += source * sun_disc * weight;
        weight *= clamp(volume.screen.z, 0.0, 1.0);
    }
    return volume.sun.color.rgb * illumination * volume.params.x / f32(steps);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(position_texture));
    let position = textureLoad(position_texture, vec2<i32>(in.uv * dimensions), 0);
    let mode = i32(volume.params.w + 0.5);
    var color = vec3(0.0);
    if mode == 1 || mode == 3 {
        color += raymarched_shafts(in.uv, position);
    }
    if mode == 2 || mode == 3 {
        color += screen_space_shafts(in.uv);
    }
    return vec4(color, 1.0);
}

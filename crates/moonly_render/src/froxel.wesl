@group(0) @binding(0) var position_texture: texture_2d<f32>;
@group(0) @binding(1) var<uniform> froxel: MoonlyFroxelUniform;
@group(0) @binding(2) var shadow_near: texture_depth_2d;
@group(0) @binding(3) var shadow_middle: texture_depth_2d;
@group(0) @binding(4) var shadow_far: texture_depth_2d;
@group(0) @binding(5) var shadow_sampler: sampler_comparison;
@group(0) @binding(6) var density_texture: texture_3d<f32>;

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

struct MoonlyFroxelUniform {
    sun: MoonlySunUniform,
    world_from_clip: mat4x4<f32>,
    grid_origin: vec4<f32>,
    grid_extent: vec4<f32>,
    params: vec4<f32>,
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
        ndc.z + froxel.sun.direction_bias.w,
    );
}

fn visibility(world_position: vec3<f32>, view_depth: f32) -> f32 {
    if view_depth < froxel.sun.splits_far.x {
        return cascade_visibility(shadow_near, froxel.sun.shadow_near_from_world, world_position);
    }
    if view_depth < froxel.sun.splits_far.y {
        return cascade_visibility(shadow_middle, froxel.sun.shadow_middle_from_world, world_position);
    }
    if view_depth < froxel.sun.splits_far.z {
        return cascade_visibility(shadow_far, froxel.sun.shadow_far_from_world, world_position);
    }
    return 0.0;
}

fn grid_coordinates(world_position: vec3<f32>) -> vec3<i32> {
    let normalized = (world_position - froxel.grid_origin.xyz) / froxel.grid_extent.xyz;
    let dimensions = vec3<f32>(textureDimensions(density_texture));
    return vec3<i32>(normalized * dimensions);
}

fn inside_grid(world_position: vec3<f32>) -> bool {
    let normalized = (world_position - froxel.grid_origin.xyz) / froxel.grid_extent.xyz;
    return all(normalized >= vec3(0.0)) && all(normalized < vec3(1.0));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(position_texture));
    let position = textureLoad(position_texture, vec2<i32>(in.uv * dimensions), 0);
    let ndc = vec4(in.uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 1.0, 1.0);
    let far = froxel.world_from_clip * ndc;
    var end_position = far.xyz / far.w;
    if position.w > 0.5 {
        end_position = position.xyz;
    }
    let ray = end_position - froxel.sun.view_position.xyz;
    let ray_length = min(length(ray), 80.0);
    if ray_length < 0.001 {
        return vec4(0.0);
    }
    let direction = ray / max(length(ray), 0.001);
    let steps = i32(clamp(froxel.params.y, 4.0, 64.0));
    let step_length = ray_length / f32(steps);
    var transmittance = 1.0;
    var scattering = vec3(0.0);
    for (var index = 0; index < steps; index += 1) {
        let distance = (f32(index) + 0.5) * step_length;
        let point = froxel.sun.view_position.xyz + direction * distance;
        if inside_grid(point) {
            let cell = textureLoad(density_texture, grid_coordinates(point), 0);
            let density = max(cell.a * froxel.params.x, 0.0);
            let optical_depth = density * step_length;
            let light = froxel.sun.color.rgb * visibility(point, distance);
            scattering += transmittance * cell.rgb * light * optical_depth;
            transmittance *= exp(-optical_depth);
        }
    }
    return vec4(scattering, transmittance);
}

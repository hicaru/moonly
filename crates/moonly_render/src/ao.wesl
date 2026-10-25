@group(0) @binding(0) var position_texture: texture_2d<f32>;
@group(0) @binding(1) var normal_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> params: MoonlyAoUniform;

struct MoonlyAoUniform {
    clip_from_world: mat4x4<f32>,
    view_from_world: mat4x4<f32>,
    settings: vec4<f32>,
    texel: vec4<f32>,
};

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

const DIRECTIONS = array<vec2<f32>, 8>(
    vec2(1.0, 0.0), vec2(-1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, -1.0),
    vec2(0.7071, 0.7071), vec2(-0.7071, 0.7071),
    vec2(0.7071, -0.7071), vec2(-0.7071, -0.7071),
);

fn load_position(uv: vec2<f32>, dimensions: vec2<f32>) -> vec4<f32> {
    return textureLoad(position_texture, vec2<i32>(uv * dimensions), 0);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    if params.settings.w < 0.5 {
        return vec4(1.0);
    }
    let dimensions = vec2<f32>(textureDimensions(position_texture));
    let pixel = vec2<i32>(in.uv * dimensions);
    let position = textureLoad(position_texture, pixel, 0);
    if position.w < 0.5 {
        return vec4(1.0);
    }
    let normal = normalize(textureLoad(normal_texture, pixel, 0).xyz);
    let reference = select(vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), abs(normal.y) > 0.95);
    let tangent = normalize(cross(reference, normal));
    let bitangent = cross(normal, tangent);
    let radius = max(params.settings.x, 0.001);
    let rings = select(3, 2, params.settings.w < 2.5);
    var occlusion = 0.0;
    var samples = 0.0;
    for (var direction_index = 0; direction_index < 8; direction_index += 1) {
        let direction = DIRECTIONS[direction_index];
        for (var ring = 1; ring <= rings; ring += 1) {
            let distance = radius * f32(ring) / f32(rings);
            let offset = (tangent * direction.x + bitangent * direction.y) * distance;
            let sample_position = position.xyz + offset + normal * distance * 0.35;
            let clip = params.clip_from_world * vec4(sample_position, 1.0);
            if clip.w <= 0.0 {
                continue;
            }
            let uv = clip.xy / clip.w * vec2(0.5, -0.5) + vec2(0.5);
            if any(uv < vec2(0.0)) || any(uv > vec2(1.0)) {
                continue;
            }
            let actual = load_position(uv, dimensions);
            if actual.w < 0.5 {
                continue;
            }
            let expected_view = params.view_from_world * vec4(sample_position, 1.0);
            let actual_view = params.view_from_world * vec4(actual.xyz, 1.0);
            let range_weight = 1.0 - clamp(length(actual.xyz - position.xyz) / radius, 0.0, 1.0);
            let depth_bias = max(params.settings.y, distance * 0.25);
            let blocked = actual_view.z > expected_view.z + depth_bias;
            occlusion += select(0.0, range_weight, blocked);
            samples += 1.0;
        }
    }
    let ao = 1.0 - clamp(occlusion / max(samples, 1.0) * params.settings.z, 0.0, 1.0);
    return vec4(ao, ao, ao, ao);
}

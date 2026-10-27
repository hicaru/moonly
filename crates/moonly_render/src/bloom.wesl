@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var<uniform> bloom: MoonlyBloomUniform;

struct MoonlyBloomUniform {
    input_size: vec2<f32>,
    output_size: vec2<f32>,
    mode: vec4<u32>,
    params: vec4<f32>,
};

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn sample_source(uv: vec2<f32>) -> vec3<f32> {
    let dimensions = vec2<f32>(textureDimensions(source_texture));
    let pixel = vec2<i32>(clamp(uv, vec2(0.0), vec2(0.99999)) * dimensions);
    let exposure = select(1.0, bloom.params.x, bloom.mode.x == 0u && bloom.params.w < 0.5);
    return textureLoad(source_texture, pixel, 0).rgb * exposure;
}

fn karis_weight(color: vec3<f32>) -> f32 {
    let luma = max(dot(color, vec3(0.2126, 0.7152, 0.0722)), 0.0);
    return 1.0 / (1.0 + luma);
}

fn bright(color: vec3<f32>) -> vec3<f32> {
    let luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    let threshold = max(bloom.params.y, 0.0);
    let weight = clamp(luma - threshold, 0.0, 8.0) / max(luma, 0.001);
    if bloom.params.w >= 0.5 && bloom.params.z <= 0.0 {
        return color * weight;
    }
    let softness = max(bloom.params.z, 0.0001);
    let soft = smoothstep(threshold - softness, threshold + softness, luma);
    return color * weight * soft;
}

@fragment
fn downsample(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / bloom.input_size;
    let uv = in.uv;
    let a = sample_source(uv + vec2(-2.0, 2.0) * texel);
    let b = sample_source(uv + vec2(0.0, 2.0) * texel);
    let c = sample_source(uv + vec2(2.0, 2.0) * texel);
    let d = sample_source(uv + vec2(-1.0, 1.0) * texel);
    let e = sample_source(uv + vec2(1.0, 1.0) * texel);
    let f = sample_source(uv + vec2(-2.0, 0.0) * texel);
    let g = sample_source(uv);
    let h = sample_source(uv + vec2(2.0, 0.0) * texel);
    let i = sample_source(uv + vec2(-1.0, -1.0) * texel);
    let j = sample_source(uv + vec2(1.0, -1.0) * texel);
    let k = sample_source(uv + vec2(-2.0, -2.0) * texel);
    let l = sample_source(uv + vec2(0.0, -2.0) * texel);
    let m = sample_source(uv + vec2(2.0, -2.0) * texel);
    let groups = array<vec3<f32>, 5>(
        (a + b + d + e) * 0.25,
        (b + c + e + f) * 0.25,
        (d + e + g + h) * 0.25,
        (e + f + h + i) * 0.25,
        (j + k + l + m) * 0.25,
    );
    var weighted = vec3(0.0);
    var weight_sum = 0.0;
    for (var index = 0; index < 5; index += 1) {
        let weight = karis_weight(groups[index]);
        weighted += groups[index] * weight;
        weight_sum += weight;
    }
    var color = weighted / max(weight_sum, 0.00000001);
    if bloom.mode.x == 0u {
        color = bright(color);
    }
    return vec4(color, 1.0);
}

@fragment
fn upsample(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / bloom.input_size;
    var color = sample_source(in.uv) * 4.0;
    color += sample_source(in.uv + vec2(-1.0, -1.0) * texel);
    color += sample_source(in.uv + vec2(1.0, -1.0) * texel);
    color += sample_source(in.uv + vec2(-1.0, 1.0) * texel);
    color += sample_source(in.uv + vec2(1.0, 1.0) * texel);
    return vec4(color / 8.0, 1.0);
}

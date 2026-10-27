@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var<uniform> reduce: MoonlyLuminanceUniform;

struct MoonlyLuminanceUniform {
    input_size: vec2<f32>,
    output_size: vec2<f32>,
    mode: vec4<u32>,
};

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn sample_value(uv: vec2<f32>) -> f32 {
    let input_size = vec2<f32>(textureDimensions(source_texture));
    let pixel = vec2<i32>(clamp(uv, vec2(0.0), vec2(0.99999)) * input_size);
    let color = textureLoad(source_texture, pixel, 0).rgb;
    if reduce.mode.x == 0u {
        return log2(max(dot(color, vec3(0.2126, 0.7152, 0.0722)), 0.0001));
    }
    return color.r;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let base = floor(in.uv * reduce.output_size) * 4.0;
    var sum = 0.0;
    for (var y = 0u; y < 4u; y += 1u) {
        for (var x = 0u; x < 4u; x += 1u) {
            let sample_pixel = base + vec2(f32(x), f32(y)) + vec2(0.5);
            sum += sample_value(sample_pixel / (reduce.output_size * 4.0));
        }
    }
    let average = sum / 16.0;
    if reduce.mode.x == 3u {
        return vec4(exp2(average), 0.0, 0.0, 1.0);
    }
    return vec4(average, 0.0, 0.0, 1.0);
}

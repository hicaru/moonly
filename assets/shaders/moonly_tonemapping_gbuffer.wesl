import bevy_pbr::prepass::io::VertexOutput;
import bevy_render::maths::PI;

struct MoonlyGbufferOutput {
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) albedo_gloss: vec4<f32>,
    @location(3) accumulator: vec4<f32>,
    @location(4) emissive: vec4<f32>,
};

fn color_sweep(uv_input: vec2<f32>) -> vec3<f32> {
    var uv = uv_input;
    let steps = 24.0;
    uv.y = uv.y * (1.0 + 1.0 / steps);
    let ratio = 2.0;
    let h = PI * 2.0 * floor(1.0 + steps * uv.y) / steps;
    let luminance = floor(uv.x * steps * ratio) / (steps * ratio) - 0.5;
    var color = vec3(0.0);
    if uv.y < 1.0 {
        color = cos(h + vec3(0.0, 1.0, 2.0) * PI * 2.0 / 3.0);
        let maximum = max(color.r, max(color.g, color.b));
        let minimum = min(color.r, min(color.g, color.b));
        color = exp(15.0 * luminance) * (color - minimum) / (maximum - minimum);
    } else {
        color = vec3(exp(15.0 * luminance));
    }
    return color;
}

fn hsv_to_srgb(color: vec3<f32>) -> vec3<f32> {
    let constants = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let offsets = abs(fract(color.xxx + constants.xyz) * 6.0 - constants.www);
    return color.z * mix(constants.xxx, clamp(offsets - constants.xxx, vec3(0.0), vec3(1.0)), color.y);
}

fn continuous_hue(uv: vec2<f32>) -> vec3<f32> {
    return hsv_to_srgb(vec3(uv.x, 1.0, 1.0)) * max(0.0, exp2(uv.y * 9.0) - 1.0);
}

@fragment
fn fragment(in: VertexOutput) -> MoonlyGbufferOutput {
    var uv = in.uv;
    var color = vec3(0.0);
    if uv.y > 0.5 {
        uv.y = 1.0 - uv.y;
        color = color_sweep(vec2(uv.x, uv.y * 2.0));
    } else {
        color = continuous_hue(vec2(uv.y * 2.0, uv.x));
    }
    var out: MoonlyGbufferOutput;
    out.position = vec4(in.world_position.xyz, 1.0);
    out.normal = vec4(in.world_normal, 0.375);
    out.albedo_gloss = vec4(0.0, 0.0, 0.0, 1.0);
    out.accumulator = vec4(color, 1.0);
    out.emissive = vec4(color, 1.0);
    return out;
}

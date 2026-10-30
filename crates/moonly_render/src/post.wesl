import moonly_post::tonemap::evaluate_hdr10_tonemap;
import moonly_post::bloom_optics::{
    apply_heat_vision_wesl,
    apply_night_vision_wesl,
    apply_parallax_scope_wesl,
};

@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var bloom_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> post: MoonlyPostUniform;

struct MoonlyPostUniform {
    exposure: vec4<f32>,
    bloom: vec4<f32>,
    modes: vec4<u32>,
    display: vec4<f32>,
    grading: vec4<f32>,
};

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn load_scene(pixel: vec2<i32>, dimensions: vec2<i32>) -> vec3<f32> {
    let bounded = clamp(pixel, vec2(0), dimensions - vec2(1));
    return textureLoad(scene_texture, bounded, 0).rgb;
}

fn bright(color: vec3<f32>) -> vec3<f32> {
    let luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    return color * clamp(luma - 1.0, 0.0, 8.0) / max(luma, 0.001);
}

fn lens_flare(pixel: vec2<i32>, dimensions: vec2<i32>) -> vec3<f32> {
    let ghost_a = bright(load_scene(dimensions - pixel, dimensions));
    let ghost_b = bright(load_scene(dimensions / 2 + (dimensions / 2 - pixel) / 2, dimensions));
    let streak = bright(load_scene(vec2(dimensions.x - pixel.x, pixel.y), dimensions));
    return (ghost_a * vec3(0.22, 0.34, 0.48)
        + ghost_b * vec3(0.32, 0.22, 0.12)
        + streak * vec3(0.08, 0.12, 0.18)) * post.bloom.y;
}

fn optics(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    if post.modes.y == 1u {
        return apply_night_vision_wesl(color, uv, false);
    }
    if post.modes.y == 2u {
        return apply_night_vision_wesl(color, uv, true);
    }
    if post.modes.y == 3u {
        return apply_heat_vision_wesl(color);
    }
    if post.modes.y == 4u {
        return apply_parallax_scope_wesl(color, uv, vec2(0.0));
    }
    return color;
}

fn chromatic_scene(pixel: vec2<i32>, dimensions: vec2<i32>, uv: vec2<f32>) -> vec3<f32> {
    let offset = vec2<i32>((uv - vec2(0.5)) * post.grading.z * 64.0);
    let red = load_scene(pixel + offset, dimensions).r;
    let green = load_scene(pixel, dimensions).g;
    let blue = load_scene(pixel - offset, dimensions).b;
    return vec3<f32>(red, green, blue);
}

fn color_grade(color: vec3<f32>) -> vec3<f32> {
    let white_balance = vec3<f32>(
        1.0 + post.grading.x * 0.1,
        1.0 + post.grading.y * 0.1,
        1.0 - post.grading.x * 0.1,
    );
    return max(color * white_balance * exp2(post.exposure.w), vec3<f32>(0.0));
}

fn vignette(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32> {
    let distance_from_center = length(uv - vec2(0.5)) * 1.41421356;
    let edge = smoothstep(
        post.display.z,
        post.display.z + max(post.display.w, 0.0001),
        distance_from_center,
    );
    return color * (1.0 - post.display.y * edge);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<i32>(textureDimensions(scene_texture));
    let pixel = vec2<i32>(in.uv * vec2<f32>(dimensions));
    let scene = chromatic_scene(pixel, dimensions, in.uv);
    if post.modes.z == 1u {
        return textureLoad(scene_texture, pixel, 0);
    }
    let bloom_pixel = vec2<i32>(in.uv * vec2<f32>(textureDimensions(bloom_texture)));
    let bloom_exposure = select(1.0, post.exposure.x, post.modes.w == 1u);
    let bloom = textureLoad(bloom_texture, bloom_pixel, 0).rgb * post.bloom.x * bloom_exposure;
    let flare = lens_flare(pixel, dimensions);
    var color = (scene + flare) * post.exposure.x + bloom;
    color = (color - vec3(0.18)) * (1.0 + post.exposure.y) + vec3(0.18);
    color = evaluate_hdr10_tonemap(color, post.modes.x);
    let luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    color = mix(vec3(luma), color, post.exposure.z);
    color = vignette(optics(color_grade(color), in.uv), in.uv);
    let coverage = textureLoad(scene_texture, pixel, 0).a;
    return vec4(color, coverage);
}

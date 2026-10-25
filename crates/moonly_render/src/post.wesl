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

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let dimensions = vec2<i32>(textureDimensions(scene_texture));
    let pixel = vec2<i32>(in.uv * vec2<f32>(dimensions));
    let scene = load_scene(pixel, dimensions);
    if post.modes.z == 1u {
        return textureLoad(scene_texture, pixel, 0);
    }
    let bloom_pixel = vec2<i32>(in.uv * vec2<f32>(textureDimensions(bloom_texture)));
    let bloom = textureLoad(bloom_texture, bloom_pixel, 0).rgb * post.bloom.x;
    let flare = lens_flare(pixel, dimensions);
    var color = (scene + bloom + flare) * post.exposure.x;
    color = (color - vec3(0.18)) * (1.0 + post.exposure.y) + vec3(0.18);
    color = evaluate_hdr10_tonemap(color, post.modes.x);
    let luma = dot(color, vec3(0.2126, 0.7152, 0.0722));
    color = mix(vec3(luma), color, 1.0 + post.exposure.z);
    let coverage = textureLoad(scene_texture, pixel, 0).a;
    return vec4(optics(color, in.uv), coverage);
}

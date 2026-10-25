@group(0) @binding(0) var source_depth: texture_depth_2d;

struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@fragment
fn fragment(in: FullscreenVertexOutput) -> @builtin(frag_depth) f32 {
    let dimensions = vec2<f32>(textureDimensions(source_depth));
    let pixel = vec2<i32>(in.uv * dimensions);
    return textureLoad(source_depth, pixel, 0);
}

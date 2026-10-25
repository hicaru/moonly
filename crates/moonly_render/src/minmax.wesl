@group(0) @binding(0) var source_depth: texture_depth_2d;
@group(0) @binding(1) var destination_minmax: texture_storage_2d<rg32float, write>;
@group(0) @binding(2) var<uniform> settings: MinMaxSettings;

struct MinMaxSettings {
    source_size: u32,
};

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) invocation: vec3<u32>) {
    let output_size = vec2<u32>(textureDimensions(destination_minmax));
    if invocation.x >= output_size.x || invocation.y >= output_size.y {
        return;
    }
    let base = invocation.xy * 4u;
    var minimum = 1.0;
    var maximum = 0.0;
    for (var y = 0u; y < 4u; y += 1u) {
        for (var x = 0u; x < 4u; x += 1u) {
            let pixel = min(base + vec2<u32>(x, y), vec2<u32>(settings.source_size - 1u));
            let depth = textureLoad(source_depth, pixel, 0);
            minimum = min(minimum, depth);
            maximum = max(maximum, depth);
        }
    }
    textureStore(destination_minmax, invocation.xy, vec4(minimum, maximum, 0.0, 0.0));
}

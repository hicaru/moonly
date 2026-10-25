import bevy_pbr::{
    prepass::io::{Vertex, VertexOutput, decompress_vertex},
    render::{
        mesh_functions, skinning,
        pbr_prepass_functions::prepass_sample_color_and_alpha_discard,
    },
};

@group(4) @binding(0) var<uniform> clip_from_world: mat4x4<f32>;

@vertex
fn vertex(input: Vertex) -> VertexOutput {
    let vertex = decompress_vertex(input, input.instance_index);
    var world_from_local = mesh_functions::get_world_from_local(input.instance_index);
@if(SKINNED) {
    world_from_local = skinning::skin_model(
        vertex.joint_indices,
        vertex.joint_weights,
        input.instance_index,
    );
}
    var out: VertexOutput;
    out.world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4(vertex.position, 1.0),
    );
    out.position = clip_from_world * out.world_position;
@if(VERTEX_UVS_A) {
    out.uv = vertex.uv;
}
@if(VERTEX_UVS_B) {
    out.uv_b = vertex.uv_b;
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX) {
    out.instance_index = input.instance_index;
}
    return out;
}

@fragment
fn fragment(in: VertexOutput) {
    prepass_sample_color_and_alpha_discard(in);
}

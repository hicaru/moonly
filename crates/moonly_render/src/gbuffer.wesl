import bevy_pbr::{
    prepass::io::VertexOutput,
    render::{
        pbr_fragment::pbr_input_from_standard_material,
        pbr_functions::alpha_discard,
    },
};
import package::gbuffer_surface::{MoonlyGbufferOutput, moonly_gbuffer_output};

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> MoonlyGbufferOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material.flags,
        pbr_input.material.alpha_cutoff,
        pbr_input.material.base_color,
    );
    return moonly_gbuffer_output(in.world_position.xyz, pbr_input);
}

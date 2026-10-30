use bevy::{
    asset::{AssetServer, Handle},
    core_pipeline::{
        FullscreenShader,
        tonemapping::{DebandDither, Tonemapping, get_lut_bind_group_layout_entries},
    },
    prelude::*,
    render::{
        render_resource::{
            BindGroupLayoutDescriptor, ColorTargetState, ColorWrites,
            DynamicBindGroupLayoutEntries, FragmentState, RenderPipelineDescriptor, Sampler,
            SamplerDescriptor, ShaderStages, SpecializedRenderPipeline, TextureFormat,
            TextureSampleType,
            binding_types::{sampler, texture_2d, uniform_buffer},
        },
        renderer::RenderDevice,
        view::ViewUniform,
    },
    shader::{Shader, ShaderDefVal},
};

#[derive(Resource)]
pub(crate) struct MoonlyNativePostPipeline {
    pub tonemap_layout: BindGroupLayoutDescriptor,
    pub tonemap_sampler: Sampler,
    fullscreen: FullscreenShader,
    fragment_shader: Handle<Shader>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MoonlyNativeTonemapKey {
    pub target_format: TextureFormat,
    pub deband_dither: DebandDither,
    pub tonemapping: Tonemapping,
    pub hue_rotate: bool,
    pub white_balance: bool,
    pub sectional_color_grading: bool,
}

pub(crate) fn init_native_post_pipeline(
    mut commands: Commands,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
) {
    let mut entries = DynamicBindGroupLayoutEntries::new_with_indices(
        ShaderStages::FRAGMENT,
        (
            (0, uniform_buffer::<ViewUniform>(true)),
            (
                1,
                texture_2d(TextureSampleType::Float { filterable: false }),
            ),
            (
                2,
                sampler(bevy::render::render_resource::SamplerBindingType::NonFiltering),
            ),
        ),
    );
    let [lut_texture, lut_sampler] = get_lut_bind_group_layout_entries();
    entries = entries.extend_with_indices(((3, lut_texture), (4, lut_sampler)));
    commands.insert_resource(MoonlyNativePostPipeline {
        tonemap_layout: BindGroupLayoutDescriptor::new("moonly_native_tonemap_layout", &entries),
        tonemap_sampler: device.create_sampler(&SamplerDescriptor::default()),
        fullscreen: fullscreen.clone(),
        fragment_shader: asset_server
            .load("embedded://bevy_core_pipeline/tonemapping/tonemapping_frag.wesl"),
    });
}

impl SpecializedRenderPipeline for MoonlyNativePostPipeline {
    type Key = MoonlyNativeTonemapKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let mut shader_defs = vec![
            ShaderDefVal::UInt("TONEMAPPING_LUT_TEXTURE_BINDING_INDEX".into(), 3),
            ShaderDefVal::UInt("TONEMAPPING_LUT_SAMPLER_BINDING_INDEX".into(), 4),
        ];
        if key.deband_dither == DebandDither::Enabled {
            shader_defs.push("DEBAND_DITHER".into());
        }
        if key.hue_rotate {
            shader_defs.push("HUE_ROTATE".into());
        }
        if key.white_balance {
            shader_defs.push("WHITE_BALANCE".into());
        }
        if key.sectional_color_grading {
            shader_defs.push("SECTIONAL_COLOR_GRADING".into());
        }
        shader_defs.push(tonemap_shader_def(key.tonemapping).into());
        RenderPipelineDescriptor {
            label: Some("moonly_native_tonemapping_pipeline".into()),
            layout: vec![self.tonemap_layout.clone()],
            vertex: self.fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: self.fragment_shader.clone(),
                shader_defs,
                targets: vec![Some(ColorTargetState {
                    format: key.target_format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        }
    }
}

const fn tonemap_shader_def(tonemapping: Tonemapping) -> &'static str {
    match tonemapping {
        Tonemapping::None | Tonemapping::Linear => "TONEMAP_METHOD_LINEAR",
        Tonemapping::Reinhard => "TONEMAP_METHOD_REINHARD",
        Tonemapping::ReinhardLuminance => "TONEMAP_METHOD_REINHARD_LUMINANCE",
        Tonemapping::AcesFitted => "TONEMAP_METHOD_ACES_FITTED",
        Tonemapping::AgX => "TONEMAP_METHOD_AGX",
        Tonemapping::SomewhatBoringDisplayTransform => {
            "TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM"
        }
        Tonemapping::TonyMcMapface => "TONEMAP_METHOD_TONY_MC_MAPFACE",
        Tonemapping::BlenderFilmic => "TONEMAP_METHOD_BLENDER_FILMIC",
        Tonemapping::KhronosPbrNeutral => "TONEMAP_METHOD_PBR_NEUTRAL",
    }
}

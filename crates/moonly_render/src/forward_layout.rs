use bevy::{
    prelude::*,
    render::{
        render_resource::{
            BindGroupLayoutDescriptor, BindGroupLayoutEntries, Sampler, SamplerBindingType,
            SamplerDescriptor, ShaderStages, ShaderType, TextureSampleType,
            binding_types::{sampler, storage_buffer_read_only_sized, texture_2d, uniform_buffer},
        },
        renderer::RenderDevice,
    },
};

use crate::sun_accumulate::MoonlySunUniform;

#[derive(ShaderType, Clone, Copy, Default)]
pub(crate) struct MoonlyForwardParams {
    pub(crate) ambient: Vec4,
    pub(crate) counts: UVec4,
}

#[derive(Resource)]
pub(crate) struct MoonlyForwardLayout {
    pub(crate) layout: BindGroupLayoutDescriptor,
    pub(crate) shadow_sampler: Sampler,
}

impl MoonlyForwardLayout {
    fn new(device: &RenderDevice) -> Self {
        Self {
            layout: moonly_forward_layout(),
            shadow_sampler: device.create_sampler(&SamplerDescriptor {
                compare: Some(bevy::render::render_resource::CompareFunction::LessEqual),
                ..default()
            }),
        }
    }
}

pub(crate) fn init_forward_layout(mut commands: Commands, device: Res<RenderDevice>) {
    commands.insert_resource(MoonlyForwardLayout::new(&device));
}

#[must_use]
pub fn moonly_forward_layout() -> BindGroupLayoutDescriptor {
    moonly_forward_layout_for(ShaderStages::FRAGMENT)
}

#[must_use]
pub fn moonly_forward_layout_for(stage: ShaderStages) -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "moonly_forward_layout",
        &BindGroupLayoutEntries::sequential(
            stage,
            (
                storage_buffer_read_only_sized(false, None),
                uniform_buffer::<MoonlyForwardParams>(false),
                texture_2d(TextureSampleType::Depth),
                sampler(SamplerBindingType::Comparison),
                texture_2d(TextureSampleType::Depth),
                texture_2d(TextureSampleType::Depth),
                texture_2d(TextureSampleType::Depth),
                sampler(SamplerBindingType::Comparison),
                uniform_buffer::<MoonlySunUniform>(false),
            ),
        ),
    )
}

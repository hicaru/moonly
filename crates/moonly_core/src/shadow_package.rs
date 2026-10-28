use bevy::{
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
};

use crate::SpotShadowXform;

/// Square shadow viewport in atlas texels.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct MoonlyShadowRect {
    pub x: u32,
    pub y: u32,
    pub size: u32,
}

/// Prepared light transform and atlas allocation.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct MoonlyShadowView {
    pub light_index: usize,
    pub xform: SpotShadowXform,
    pub rect: MoonlyShadowRect,
}

/// One atlas clear and a set of shadow views.
#[derive(Debug, Default, Clone, Reflect)]
pub struct MoonlyShadowBatch {
    pub views: Vec<MoonlyShadowView>,
}

/// Shadow allocations prepared for render extraction.
#[derive(Resource, Debug, Default, Clone, Reflect)]
#[reflect(Resource)]
pub struct MoonlyShadowPackage {
    pub pool_size: u32,
    pub batches: Vec<MoonlyShadowBatch>,
}

impl ExtractResource<RenderApp> for MoonlyShadowPackage {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// One sun cascade transform and distance interval.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct MoonlySunCascade {
    pub view: MoonlyShadowView,
    pub prev_combine: Mat4,
    pub split_near: f32,
    pub split_far: f32,
    pub depth_bias: f32,
    pub blend_band: f32,
}

/// Prepared sun cascades for render extraction.
#[derive(Resource, Debug, Default, Clone, Reflect)]
#[reflect(Resource)]
pub struct MoonlySunPackage {
    pub pool_size: u32,
    pub depth_bias: f32,
    pub use_minmax_near: bool,
    pub cascades: Vec<MoonlySunCascade>,
}

impl ExtractResource<RenderApp> for MoonlySunPackage {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

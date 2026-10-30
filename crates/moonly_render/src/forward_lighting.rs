use bevy::{
    app::SubApp,
    core_pipeline::core_3d::Transparent3d,
    ecs::{query::ROQueryItem, system::SystemParam},
    pbr::{
        DrawMesh, SetMaterialBindGroup, SetMeshBindGroup, SetMeshViewBindGroup,
        SetMeshViewBindingArrayBindGroup, Transmissive3d,
    },
    prelude::*,
    render::{
        render_phase::{
            PhaseItem, RenderCommand, RenderCommandResult, SetItemPipeline, TrackedRenderPass,
        },
        render_resource::{
            BindGroup, BindGroupEntries, PipelineCache, ShaderType, TextureView, UniformBuffer,
        },
        renderer::{RenderDevice, RenderQueue},
        view::ExtractedView,
    },
};
use moonly_core::{
    LightPackage, MoonlyLight, MoonlyLightFlags, MoonlyLightType, MoonlySunPackage,
    WeatherEnvironment,
};

use crate::{
    forward_layout::{MoonlyForwardLayout, MoonlyForwardParams},
    light_packages::MoonlyViewLightPackage,
    shadow_targets::{MoonlyShadowSample, MoonlySpotShadowTarget},
    sun_accumulate::MoonlySunUniform,
};

#[derive(ShaderType, Clone, Copy)]
pub(crate) struct MoonlyForwardLight {
    position_range: Vec4,
    direction_cos: Vec4,
    color: Vec4,
    shadow_from_world: Mat4,
    shadow_settings: Vec4,
    shadow_transform: Vec4,
    misc: Vec4,
}

#[derive(Component)]
pub struct MoonlyForwardView {
    lights: bevy::render::render_resource::StorageBuffer<Vec<MoonlyForwardLight>>,
    params: UniformBuffer<MoonlyForwardParams>,
    sun: UniformBuffer<MoonlySunUniform>,
    bind_group: Option<BindGroup>,
}

impl Default for MoonlyForwardView {
    fn default() -> Self {
        Self {
            lights: bevy::render::render_resource::StorageBuffer::from(Vec::new()),
            params: UniformBuffer::from(MoonlyForwardParams {
                ambient: Vec4::ZERO,
                counts: UVec4::ZERO,
            }),
            sun: UniformBuffer::from(MoonlySunUniform::default()),
            bind_group: None,
        }
    }
}

impl MoonlyForwardView {
    #[must_use]
    pub const fn bind_group(&self) -> Option<&BindGroup> {
        self.bind_group.as_ref()
    }
}

pub(crate) type ForwardViewQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static MoonlyViewLightPackage,
        &'static MoonlySpotShadowTarget,
        &'static ExtractedView,
        Option<&'static mut MoonlyForwardView>,
    ),
    With<crate::MoonlyCamera>,
>;

#[derive(SystemParam)]
pub(crate) struct ForwardPrepareInputs<'w> {
    pub(crate) device: Res<'w, RenderDevice>,
    pub(crate) queue: Res<'w, RenderQueue>,
    pub(crate) layout: Res<'w, MoonlyForwardLayout>,
    pub(crate) environment: Res<'w, WeatherEnvironment>,
    pub(crate) cache: Res<'w, PipelineCache>,
    pub(crate) cascades: Res<'w, MoonlySunPackage>,
}

pub(crate) struct ForwardBindings<'a> {
    pub(crate) device: &'a RenderDevice,
    pub(crate) queue: &'a RenderQueue,
    pub(crate) layout: &'a MoonlyForwardLayout,
    pub(crate) cache: &'a PipelineCache,
    pub(crate) environment: &'a WeatherEnvironment,
    pub(crate) camera: &'a ExtractedView,
    pub(crate) package: &'a LightPackage,
    pub(crate) shadows: &'a MoonlySpotShadowTarget,
    pub(crate) cascades: &'a MoonlySunPackage,
}

pub(crate) fn fill_forward_view(
    view: &mut MoonlyForwardView,
    bindings: &ForwardBindings,
    depth: Option<&TextureView>,
) {
    let package = bindings.package;
    let shadows = bindings.shadows;
    let lights: Vec<MoonlyForwardLight> = package
        .v_point
        .iter()
        .chain(package.v_spot.iter())
        .chain(package.v_shadowed.iter())
        .map(|light| forward_light(light, shadows.sample(light).as_ref()))
        .chain(
            package
                .v_direct
                .iter()
                .map(|light| forward_light(light, None)),
        )
        .collect();
    let count = u32::try_from(lights.len()).unwrap_or(u32::MAX);
    let direct = package.v_direct.first();
    let sun_enabled = direct.is_some_and(|light| light.flags.contains(MoonlyLightFlags::SHADOW))
        && shadows.sun_batches.len() >= 3;
    let sun = direct.map_or_else(MoonlySunUniform::default, |light| {
        MoonlySunUniform::new(light, bindings.camera, &bindings.cascades.cascades, shadows)
    });
    view.lights.set(lights);
    view.lights.write_buffer(bindings.device, bindings.queue);
    view.params = UniformBuffer::from(MoonlyForwardParams {
        ambient: bindings.environment.ambient_color.extend(1.0),
        counts: UVec4::new(count, u32::from(sun_enabled), 0, 0),
    });
    view.params.write_buffer(bindings.device, bindings.queue);
    view.sun = UniformBuffer::from(sun);
    view.sun.write_buffer(bindings.device, bindings.queue);
    let sun_depth = |index: usize| {
        shadows
            .sun_batches
            .get(index)
            .map(|batch| &batch.depth.texture.default_view)
            .or(depth)
    };
    view.bind_group = depth.map(|depth| {
        bindings.device.create_bind_group(
            Some("moonly_forward_bind_group"),
            &bindings
                .cache
                .get_bind_group_layout(&bindings.layout.layout),
            &BindGroupEntries::sequential((
                &view.lights,
                &view.params,
                depth,
                &bindings.layout.shadow_sampler,
                sun_depth(0).unwrap_or(depth),
                sun_depth(1).unwrap_or(depth),
                sun_depth(2).unwrap_or(depth),
                &bindings.layout.shadow_sampler,
                &view.sun,
            )),
        )
    });
}

fn forward_light(light: &MoonlyLight, shadow: Option<&MoonlyShadowSample>) -> MoonlyForwardLight {
    let enabled = shadow.is_some_and(|sample| sample.enabled);
    let rect = shadow.map_or(
        moonly_core::MoonlyShadowRect {
            x: 0,
            y: 0,
            size: 1,
        },
        |sample| sample.rect,
    );
    let atlas_size = shadow.map_or(1, |sample| sample.atlas_size);
    let kind = match light.light_type {
        MoonlyLightType::Direct => 2.0,
        MoonlyLightType::Point => 0.0,
        MoonlyLightType::Spot | MoonlyLightType::Omnipart => 1.0,
    };
    MoonlyForwardLight {
        position_range: light.position.extend(light.range),
        direction_cos: light.direction.extend(if kind == 0.0 {
            -1.0
        } else {
            (light.cone_angle * 0.5).cos()
        }),
        color: light.color.extend(0.0),
        shadow_from_world: shadow.map_or(Mat4::IDENTITY, |sample| sample.xform.combine),
        shadow_settings: Vec4::new(
            if enabled { 1.0 } else { 0.0 },
            0.0005,
            1.0 / rect.size as f32,
            kind,
        ),
        shadow_transform: Vec4::new(
            rect.x as f32 / atlas_size as f32,
            rect.y as f32 / atlas_size as f32,
            rect.size as f32 / atlas_size as f32,
            0.0,
        ),
        misc: Vec4::new(
            if light.inner_cone_angle >= 0.0 {
                (light.inner_cone_angle * 0.5).cos()
            } else {
                -1.0
            },
            if light.flags.contains(MoonlyLightFlags::NATIVE) {
                1.0
            } else {
                0.0
            },
            0.0,
            0.0,
        ),
    }
}

pub struct SetMoonlyForwardBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMoonlyForwardBindGroup<I> {
    type Param = ();
    type ViewQuery = &'static MoonlyForwardView;
    type ItemQuery = ();

    fn render<'w>(
        _item: &P,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        _param: (),
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(group) = view.bind_group.as_ref() else {
            return RenderCommandResult::Failure("Moonly forward lighting is not prepared");
        };
        pass.set_bind_group(I, group, &[]);
        RenderCommandResult::Success
    }
}

pub(crate) type DrawMoonlyForwardMaterial = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshViewBindingArrayBindGroup<1>,
    SetMeshBindGroup<2>,
    SetMaterialBindGroup<3>,
    SetMoonlyForwardBindGroup<4>,
    DrawMesh,
);

pub(crate) fn register_forward_draws(render_app: &mut SubApp) {
    use bevy::render::render_phase::AddRenderCommand;
    render_app.add_render_command::<Transmissive3d, DrawMoonlyForwardMaterial>();
    render_app.add_render_command::<Transparent3d, DrawMoonlyForwardMaterial>();
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_attenuation_matches_bevy_range_falloff() {
        let source = include_str!("forward_lighting.wesl");
        assert!(source.contains("smooth_factor * smooth_factor / max(distance_squared, 0.0001)"));
        assert!(!source.contains("light.position_range.w * 0.95"));

        let distance: f32 = 4.75;
        let range: f32 = 10.0;
        let factor = distance * distance / (range * range);
        let smooth_factor = 1.0 - factor * factor;
        let expected = smooth_factor * smooth_factor / (distance * distance);
        let actual = (1.0 - factor * factor).powi(2) / distance.powi(2);
        assert!((actual - expected).abs() < 1.0e-6);
    }
}

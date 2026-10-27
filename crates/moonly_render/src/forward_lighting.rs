use bevy::{
    app::SubApp,
    core_pipeline::core_3d::Transparent3d,
    ecs::query::ROQueryItem,
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
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages, ShaderType,
            TextureSampleType, UniformBuffer,
            binding_types::{sampler, storage_buffer_read_only_sized, texture_2d, uniform_buffer},
        },
        renderer::{RenderDevice, RenderQueue},
    },
};
use moonly_core::{
    LightPackage, MoonlyLight, MoonlyLightFlags, MoonlyLightType, WeatherEnvironment,
};

use crate::{
    light_packages::MoonlyViewLightPackage,
    shadow_targets::{MoonlyShadowSample, MoonlySpotShadowTarget},
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

#[derive(ShaderType, Clone, Copy)]
struct MoonlyForwardParams {
    ambient: Vec4,
    counts: UVec4,
}

#[derive(Component)]
pub struct MoonlyForwardView {
    lights: bevy::render::render_resource::StorageBuffer<Vec<MoonlyForwardLight>>,
    params: UniformBuffer<MoonlyForwardParams>,
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
            bind_group: None,
        }
    }
}

impl MoonlyForwardView {
    #[must_use]
    pub fn bind_group(&self) -> Option<&BindGroup> {
        self.bind_group.as_ref()
    }
}

#[derive(Resource)]
pub(crate) struct MoonlyForwardLayout {
    layout: BindGroupLayoutDescriptor,
    shadow_sampler: Sampler,
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
            ),
        ),
    )
}

type ForwardViewQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static MoonlyViewLightPackage,
        &'static MoonlySpotShadowTarget,
        Option<&'static mut MoonlyForwardView>,
    ),
    With<crate::MoonlyCamera>,
>;

pub(crate) fn prepare_forward_bind_groups(
    mut commands: Commands,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    layout: Res<MoonlyForwardLayout>,
    environment: Res<WeatherEnvironment>,
    cache: Res<bevy::render::render_resource::PipelineCache>,
    mut views: ForwardViewQuery,
) {
    for (entity, package, shadows, current) in &mut views {
        let depth = shadows.first_depth();
        if let Some(mut current) = current {
            fill_forward_view(
                &mut current,
                &device,
                &queue,
                &layout,
                &cache,
                &environment,
                &package.0,
                shadows,
                depth,
            );
            continue;
        }
        let mut current = MoonlyForwardView::default();
        fill_forward_view(
            &mut current,
            &device,
            &queue,
            &layout,
            &cache,
            &environment,
            &package.0,
            shadows,
            depth,
        );
        commands.entity(entity).insert(current);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Render preparation requires these resources"
)]
fn fill_forward_view(
    view: &mut MoonlyForwardView,
    device: &RenderDevice,
    queue: &RenderQueue,
    layout: &MoonlyForwardLayout,
    cache: &bevy::render::render_resource::PipelineCache,
    environment: &WeatherEnvironment,
    package: &LightPackage,
    shadows: &MoonlySpotShadowTarget,
    depth: Option<&bevy::render::render_resource::TextureView>,
) {
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
    let count = lights.len() as u32;
    view.lights.set(lights);
    view.lights.write_buffer(device, queue);
    view.params = UniformBuffer::from(MoonlyForwardParams {
        ambient: environment.ambient_color.extend(1.0),
        counts: UVec4::new(count, 0, 0, 0),
    });
    view.params.write_buffer(device, queue);
    view.bind_group = depth.map(|depth| {
        device.create_bind_group(
            Some("moonly_forward_bind_group"),
            &cache.get_bind_group_layout(&layout.layout),
            &BindGroupEntries::sequential((
                &view.lights,
                &view.params,
                depth,
                &layout.shadow_sampler,
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
            return RenderCommandResult::Skip;
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

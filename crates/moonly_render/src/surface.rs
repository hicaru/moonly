use bevy::{
    ecs::{
        query::{QueryItem, ROQueryItem},
        system::lifetimeless::SRes,
    },
    prelude::*,
    render::{
        RenderApp,
        extract_component::ExtractComponent,
        render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
        render_resource::{
            BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries,
            PipelineCache, ShaderStages, ShaderType, binding_types::uniform_buffer,
        },
        renderer::RenderDevice,
        sync_component::SyncComponent,
        uniform::{ComponentUniforms, DynamicUniformIndex},
    },
};
use moonly_core::{HemisphereTracker, MoonlyConfig, WeatherEnvironment};

#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct MoonlySurface {
    /// Moonly material index in `0..4`.
    pub material: f32,
}

impl Default for MoonlySurface {
    fn default() -> Self {
        Self { material: 1.0 }
    }
}

#[derive(Component, ShaderType, Clone, Copy)]
pub(crate) struct MoonlySurfaceUniform {
    values: Vec2,
    positive_cube: Vec4,
    negative_cube: Vec4,
    local: Vec4,
    ambient: Vec4,
    hemi: Vec4,
}

impl MoonlySurfaceUniform {
    fn new(
        surface: Option<&MoonlySurface>,
        tracker: Option<&HemisphereTracker>,
        wetness: f32,
        environment: &WeatherEnvironment,
    ) -> Self {
        let material = surface.map_or(1.0, |surface| surface.material);
        let mut uniform = Self {
            values: Vec2::new(material_coordinate(material), wetness),
            positive_cube: Vec4::ZERO,
            negative_cube: Vec4::ZERO,
            local: Vec4::ZERO,
            ambient: environment.ambient_color.extend(1.0),
            hemi: environment.hemi_color,
        };
        if let Some(tracker) = tracker {
            let [px, py, pz, nx, ny, nz] = tracker.hemi_cube_smooth;
            uniform.positive_cube = Vec4::new(px, py, pz, tracker.hemi_smooth);
            uniform.negative_cube = Vec4::new(nx, ny, nz, 0.0);
            uniform.local = tracker.approximate_local.extend(1.0);
        }
        uniform
    }
}

#[derive(Resource)]
pub(crate) struct MoonlySurfaceLayout(pub BindGroupLayoutDescriptor);

#[derive(Resource)]
pub(crate) struct MoonlySurfaceBindGroup(pub BindGroup);

type SurfaceQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static MoonlySurface>,
        Option<&'static HemisphereTracker>,
        Option<&'static mut MoonlySurfaceUniform>,
    ),
    With<Mesh3d>,
>;

pub(crate) fn sync_moonly_surfaces(
    mut commands: Commands,
    config: Option<Res<MoonlyConfig>>,
    environment: Option<Res<WeatherEnvironment>>,
    mut meshes: SurfaceQuery,
) {
    let Some(environment) = environment else {
        return;
    };
    for (entity, surface, tracker, uniform) in &mut meshes {
        let wetness = config.as_ref().map_or(0.0, |config| config.rain_wetness);
        let updated = MoonlySurfaceUniform::new(surface, tracker, wetness, &environment);
        if let Some(mut uniform) = uniform {
            *uniform = updated;
        } else {
            commands.entity(entity).insert(updated);
        }
    }
}

impl SyncComponent<RenderApp> for MoonlySurfaceUniform {
    type Target = Self;
}

impl ExtractComponent<RenderApp> for MoonlySurfaceUniform {
    type QueryData = &'static Self;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<Self::QueryData>) -> Option<Self::Out> {
        Some(*item)
    }
}

pub(crate) fn init_surface_layout(mut commands: Commands) {
    let layout = BindGroupLayoutDescriptor::new(
        "moonly_surface_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (uniform_buffer::<MoonlySurfaceUniform>(true),),
        ),
    );
    commands.insert_resource(MoonlySurfaceLayout(layout));
}

pub(crate) fn prepare_surface_bind_group(
    mut commands: Commands,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<MoonlySurfaceUniform>>,
    layout: Res<MoonlySurfaceLayout>,
) {
    let Some(binding) = uniforms.uniforms().binding() else {
        return;
    };
    let group = device.create_bind_group(
        Some("moonly_surface_bind_group"),
        &pipeline_cache.get_bind_group_layout(&layout.0),
        &BindGroupEntries::single(binding),
    );
    commands.insert_resource(MoonlySurfaceBindGroup(group));
}

pub(crate) struct SetMoonlySurfaceBindGroup<const I: usize>;

impl<P: PhaseItem, const I: usize> RenderCommand<P> for SetMoonlySurfaceBindGroup<I> {
    type Param = Option<SRes<MoonlySurfaceBindGroup>>;
    type ViewQuery = ();
    type ItemQuery = &'static DynamicUniformIndex<MoonlySurfaceUniform>;

    fn render<'w>(
        _item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        entity: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        group: bevy::ecs::system::SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(group) = group else {
            return RenderCommandResult::Skip;
        };
        let offset = entity.map_or(0, DynamicUniformIndex::index);
        let group = group.into_inner();
        pass.set_bind_group(I, &group.0, &[offset]);
        RenderCommandResult::Success
    }
}

fn material_coordinate(material: f32) -> f32 {
    (material.clamp(0.0, 3.0) + 0.5) / 4.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_indices_map_to_moonly_lut_slice_centers() {
        for (material, expected) in [
            (0.0, 0.125),
            (1.0, 0.375),
            (2.0, 0.625),
            (3.0, 0.875),
            (9.0, 0.875),
        ] {
            assert!((material_coordinate(material) - expected).abs() < 1.0e-6);
        }
    }
}

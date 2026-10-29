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
use moonly_core::{HemisphereTracker, MoonlyConfig, MoonlyNativeForward, WeatherEnvironment};

#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component)]
pub struct MoonlySurface {
    /// Moonly material index in `0..4`.
    pub material: f32,
    /// RGB light transmission and independent shadow coverage.
    pub transmittance: Vec4,
}

impl Default for MoonlySurface {
    fn default() -> Self {
        Self {
            material: 1.0,
            transmittance: Vec4::new(0.0, 0.0, 0.0, 1.0),
        }
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
    transmittance: Vec4,
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
            transmittance: surface.map_or(Vec4::new(0.0, 0.0, 0.0, 1.0), |surface| {
                surface.transmittance
            }),
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
    native: Res<MoonlyNativeForward>,
    mut meshes: SurfaceQuery,
) {
    if native.0 {
        return;
    }
    let Some(environment) = environment else {
        return;
    };
    for (entity, surface, tracker, uniform) in &mut meshes {
        let wetness = config.as_ref().map_or(0.0, |config| config.rain_wetness);
        let updated = MoonlySurfaceUniform::new(surface, tracker, wetness, &environment);
        if let Some(mut uniform) = uniform {
            *uniform = updated;
        } else if let Ok(mut entity) = commands.get_entity(entity) {
            entity.try_insert(updated);
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
    type Param = (
        Option<SRes<MoonlySurfaceBindGroup>>,
        Query<'static, 'static, &'static DynamicUniformIndex<MoonlySurfaceUniform>>,
    );
    type ViewQuery = ();
    type ItemQuery = ();

    fn render<'w>(
        item: &P,
        _view: ROQueryItem<'w, '_, Self::ViewQuery>,
        _entity: Option<()>,
        params: bevy::ecs::system::SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let (group, indices) = params;
        let Some(group) = group else {
            return RenderCommandResult::Skip;
        };
        let offset = indices
            .get(item.entity())
            .map_or(0, DynamicUniformIndex::index);
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
    use bevy::ecs::system::SystemState;

    type SurfaceSyncState<'w, 's> = SystemState<(
        Commands<'w, 's>,
        Option<Res<'w, MoonlyConfig>>,
        Option<Res<'w, WeatherEnvironment>>,
        Res<'w, MoonlyNativeForward>,
        SurfaceQuery<'w, 's>,
    )>;

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

    #[test]
    fn opaque_and_translucent_surfaces_keep_independent_shadow_coverage() {
        let environment = WeatherEnvironment::default();
        let opaque = MoonlySurfaceUniform::new(None, None, 0.0, &environment);
        assert_eq!(opaque.transmittance, Vec4::new(0.0, 0.0, 0.0, 1.0));
        let transmittance = Vec4::new(0.8, 0.3, 0.1, 0.6);
        let surface = MoonlySurface {
            transmittance,
            ..MoonlySurface::default()
        };
        let translucent = MoonlySurfaceUniform::new(Some(&surface), None, 0.0, &environment);
        assert_eq!(translucent.transmittance, transmittance);
    }

    #[test]
    fn despawn_before_command_application_does_not_fail() {
        let mut world = World::new();
        world.insert_resource(WeatherEnvironment::default());
        world.init_resource::<MoonlyNativeForward>();
        let entity = world.spawn(Mesh3d::default()).id();
        let mut state = SurfaceSyncState::new(&mut world);
        let Ok((commands, config, environment, native, meshes)) = state.get_mut(&mut world) else {
            return;
        };
        sync_moonly_surfaces(commands, config, environment, native, meshes);
        assert!(world.despawn(entity));
        state.apply(&mut world);
    }
}

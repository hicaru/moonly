use bevy::{pbr::StandardMaterial, prelude::*};
use moonly_render::MoonlySurface;

#[derive(Component)]
struct ManagedMoonlySurface;

type NativeSurfaceQuery = (
    Entity,
    &'static MeshMaterial3d<StandardMaterial>,
    Option<&'static mut MoonlySurface>,
    Has<ManagedMoonlySurface>,
);

pub(crate) fn configure(app: &mut App) {
    app.add_systems(PreUpdate, sync_native_surfaces);
}

fn sync_native_surfaces(
    mut commands: Commands,
    materials: Option<Res<Assets<StandardMaterial>>>,
    mut meshes: Query<NativeSurfaceQuery, With<Mesh3d>>,
) {
    let Some(materials) = materials else {
        return;
    };
    for (entity, handle, current, managed) in &mut meshes {
        if current.is_some() && !managed {
            continue;
        }
        let Some(material) = materials.get(handle) else {
            continue;
        };
        if material.diffuse_transmission <= 0.0 {
            if managed {
                commands
                    .entity(entity)
                    .remove::<(MoonlySurface, ManagedMoonlySurface)>();
            }
            continue;
        }
        let adapted = native_surface(material);
        if let Some(mut current) = current {
            *current = adapted;
        } else {
            commands
                .entity(entity)
                .insert((adapted, ManagedMoonlySurface));
        }
    }
}

fn native_surface(material: &StandardMaterial) -> MoonlySurface {
    let color = material.base_color.to_linear();
    MoonlySurface {
        transmittance: Vec4::new(
            color.red,
            color.green,
            color.blue,
            material.diffuse_transmission.clamp(0.0, 1.0),
        ),
        ..MoonlySurface::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffuse_transmission_produces_colored_shadow_transmittance() {
        let material = StandardMaterial {
            base_color: Color::linear_rgb(0.2, 0.4, 0.6),
            diffuse_transmission: 0.7,
            ..StandardMaterial::default()
        };
        let surface = native_surface(&material);
        assert_eq!(surface.transmittance, Vec4::new(0.2, 0.4, 0.6, 0.7));
    }

    #[test]
    fn explicit_moonly_surface_is_not_managed() {
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>();
        app.add_systems(PreUpdate, sync_native_surfaces);
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                diffuse_transmission: 1.0,
                ..StandardMaterial::default()
            });
        let explicit = MoonlySurface {
            transmittance: Vec4::new(0.9, 0.1, 0.2, 0.3),
            ..MoonlySurface::default()
        };
        let entity = app
            .world_mut()
            .spawn((Mesh3d::default(), MeshMaterial3d(material), explicit))
            .id();
        app.update();
        assert_eq!(app.world().get::<MoonlySurface>(entity), Some(&explicit));
    }
}

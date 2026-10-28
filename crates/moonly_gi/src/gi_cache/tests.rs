use super::*;
use crate::gi_bvh::GiTriangle;

fn test_geometry() -> StaticGiGeometry {
    let vertices = [
        Vec3::new(-100.0, -1.0, -100.0),
        Vec3::new(-100.0, -1.0, 100.0),
        Vec3::new(100.0, -1.0, -100.0),
    ];
    let mut triangles = Vec::new();
    if let Some(triangle) = GiTriangle::new(vertices, true) {
        triangles.push(triangle);
    }
    let mut geometry = StaticGiGeometry::default();
    geometry.rebuild(&mut triangles);
    geometry
}

fn test_config() -> MoonlyConfig {
    let mut config = MoonlyConfig::default();
    config.gi.num_photons = 2;
    config.gi.energy_clip = 0.0;
    config
}

fn test_light() -> MoonlyLight {
    MoonlyLight {
        position: Vec3::new(0.0, 2.0, 0.0),
        flags: MoonlyLightFlags::GI_ENABLED,
        ..MoonlyLight::default()
    }
}

struct TestContext<'a> {
    config: &'a MoonlyConfig,
    geometry: &'a StaticGiGeometry,
}

fn generate(
    cache: &mut GiLightCache,
    entity: Entity,
    light: &MoonlyLight,
    test: &TestContext,
    field: &mut GiLightField,
) {
    let mut scratch = Vec::new();
    let mut context = GiGenerationContext {
        config: test.config,
        geometry: test.geometry,
        reflectors: &[],
        scratch: &mut scratch,
    };
    generate_cached_light(cache, entity, light, &mut context, field);
}

#[test]
fn unchanged_generation_casts_no_rays_and_preserves_all_cached_lights() {
    let geometry = test_geometry();
    let config = test_config();
    let light = test_light();
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    let mut cache = GiLightCache::default();
    let mut field = GiLightField::default();
    generate(
        &mut cache,
        entity,
        &light,
        &TestContext {
            config: &config,
            geometry: &geometry,
        },
        &mut field,
    );
    let first_rays = geometry.rays_traced();
    let first_lights = field.virtual_lights.clone();
    assert_eq!(first_rays, 16);
    assert_ne!(first_lights.len(), 0);

    field.virtual_lights.clear();
    generate(
        &mut cache,
        entity,
        &light,
        &TestContext {
            config: &config,
            geometry: &geometry,
        },
        &mut field,
    );
    assert_eq!(geometry.rays_traced(), first_rays);
    assert_eq!(field.virtual_lights, first_lights);
}

#[test]
fn only_the_changed_light_or_geometry_invalidates_dependent_caches() {
    let mut geometry = test_geometry();
    let config = test_config();
    let first_light = test_light();
    let mut second_light = test_light();
    second_light.position.x = 1.0;
    let mut world = World::new();
    let first = world.spawn_empty().id();
    let second = world.spawn_empty().id();
    let mut cache = GiLightCache::default();
    let mut field = GiLightField::default();
    generate(
        &mut cache,
        first,
        &first_light,
        &TestContext {
            config: &config,
            geometry: &geometry,
        },
        &mut field,
    );
    generate(
        &mut cache,
        second,
        &second_light,
        &TestContext {
            config: &config,
            geometry: &geometry,
        },
        &mut field,
    );
    let initial_rays = geometry.rays_traced();

    let mut moved = first_light.clone();
    moved.position.y += 1.0;
    generate(
        &mut cache,
        first,
        &moved,
        &TestContext {
            config: &config,
            geometry: &geometry,
        },
        &mut field,
    );
    assert_eq!(geometry.rays_traced(), initial_rays + 16);
    generate(
        &mut cache,
        second,
        &second_light,
        &TestContext {
            config: &config,
            geometry: &geometry,
        },
        &mut field,
    );
    assert_eq!(geometry.rays_traced(), initial_rays + 16);

    geometry.rebuild(&mut Vec::new());
    generate(
        &mut cache,
        first,
        &moved,
        &TestContext {
            config: &config,
            geometry: &geometry,
        },
        &mut field,
    );
    generate(
        &mut cache,
        second,
        &second_light,
        &TestContext {
            config: &config,
            geometry: &geometry,
        },
        &mut field,
    );
    assert_eq!(geometry.rays_traced(), initial_rays + 48);
    assert_eq!(cache.light_count(), 2);
}

#[test]
fn range_cone_and_config_each_invalidate_a_light() {
    let geometry = test_geometry();
    let mut config = test_config();
    let mut light = test_light();
    light.light_type = MoonlyLightType::Spot;
    let mut world = World::new();
    let entity = world.spawn_empty().id();
    let mut cache = GiLightCache::default();
    let mut field = GiLightField::default();
    generate(
        &mut cache,
        entity,
        &light,
        &TestContext {
            config: &config,
            geometry: &geometry,
        },
        &mut field,
    );
    let mut expected = 16;

    for change in 0..3 {
        match change {
            0 => light.range += 1.0,
            1 => light.cone_angle += 0.1,
            _ => config.gi.reflectivity += 0.1,
        }
        generate(
            &mut cache,
            entity,
            &light,
            &TestContext {
                config: &config,
                geometry: &geometry,
            },
            &mut field,
        );
        expected += 16;
        assert_eq!(geometry.rays_traced(), expected);
    }
}

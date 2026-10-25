//! Criterion benchmarks for static triangle BVH construction and traversal.

use std::{hint::black_box, time::Duration};

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use moonly::prelude::*;

fn build_triangles(count: usize) -> Vec<GiTriangle> {
    (0..count)
        .filter_map(|index| {
            let x = (index % 1000) as f32 * 0.9;
            let y = (index / 1000) as f32 * 0.9;
            GiTriangle::new(
                [
                    Vec3::new(x, y, 0.0),
                    Vec3::new(x + 0.8, y, 0.0),
                    Vec3::new(x, y + 0.8, 0.0),
                ],
                true,
            )
        })
        .collect()
}

fn build_bvh(triangles: &mut Vec<GiTriangle>) -> GiTriangleBvh {
    let mut bvh = GiTriangleBvh::default();
    bvh.rebuild(triangles);
    bvh
}

fn bench_bvh_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("gi_bvh_build");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    for count in [10_000, 100_000, 1_000_000] {
        let template = build_triangles(count);
        group.throughput(Throughput::Elements(count as u64));
        group.bench_function(format!("triangles_{count}"), |bencher| {
            bencher.iter_batched(
                || template.clone(),
                |mut triangles| {
                    let bvh = build_bvh(&mut triangles);
                    black_box((bvh.triangle_count(), bvh.node_count()))
                },
                criterion::BatchSize::LargeInput,
            );
        });
        let bvh = build_bvh(&mut template.clone());
        println!(
            "BVH {count} triangles: {} allocated bytes",
            bvh.allocated_bytes()
        );
    }
    group.finish();
}

fn bench_bvh_nearest(c: &mut Criterion) {
    let mut group = c.benchmark_group("gi_bvh_nearest_1024_rays");
    for count in [10_000, 100_000, 1_000_000] {
        let bvh = build_bvh(&mut build_triangles(count));
        group.throughput(Throughput::Elements(1024));
        group.bench_function(format!("triangles_{count}"), |bencher| {
            bencher.iter(|| {
                let mut hits = 0_usize;
                for index in 0..1024_usize {
                    let x = (index % 32) as f32 * 0.9 + 0.1;
                    let y = (index / 32) as f32 * 0.9 + 0.1;
                    let ray = GiRayQuery::new(Vec3::new(x, y, 5.0), Vec3::NEG_Z, 0.0001, 10.0);
                    hits += usize::from(bvh.nearest_hit(black_box(ray)).is_some());
                }
                black_box(hits)
            });
        });
    }
    group.finish();
}

fn gi_cache_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MoonlyCorePlugin, MoonlyGiPlugin));
    app.world_mut().spawn(GiReflectorBox::from_center_extents(
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(8.0, 0.5, 8.0),
        Vec3::new(0.7, 0.6, 0.4),
    ));
    app
}

fn bench_gi_cache(c: &mut Criterion) {
    let light = MoonlyLight::spot(
        Vec3::new(0.0, 4.0, 0.0),
        Vec3::NEG_Y,
        Vec3::new(3.0, 2.5, 2.0),
        16.0,
        1.2,
    );
    let mut group = c.benchmark_group("gi_cache_cold_warm");
    group.bench_function("cold_new_light", |bencher| {
        let mut app = gi_cache_app();
        bencher.iter(|| {
            let entity = app.world_mut().spawn(light.clone()).id();
            app.update();
            app.world_mut().despawn(entity);
        });
    });
    group.bench_function("warm_unchanged_frame", |bencher| {
        let mut app = gi_cache_app();
        app.world_mut().spawn(light.clone());
        app.update();
        app.update();
        bencher.iter(|| app.update());
    });
    group.finish();
}

fn light_tracking_app(count: usize) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MoonlyCorePlugin, MoonlyGiPlugin));
    app.init_resource::<Assets<Mesh>>();
    let roof = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::new(20.0, 0.2, 20.0));
    app.world_mut().spawn((
        Mesh3d(roof),
        Transform::from_xyz(0.0, 4.0, 0.0),
        MoonlyGiCaster::default(),
    ));
    for index in 0..count {
        app.world_mut().spawn((
            Transform::from_xyz(index as f32 * 0.1, 1.0, 0.0),
            HemisphereTracker::default(),
        ));
    }
    app.update();
    app
}

fn bench_light_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("light_tracking_roofed");
    group.sample_size(10);
    for count in [1, 32, 128] {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_function(format!("objects_{count}"), |bencher| {
            let mut app = light_tracking_app(count);
            bencher.iter(|| app.update());
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_bvh_build,
    bench_bvh_nearest,
    bench_gi_cache,
    bench_light_tracking
);
criterion_main!(benches);

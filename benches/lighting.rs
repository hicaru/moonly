//! Criterion benchmarks comparing Monolith R4 baseline and Moonly enhanced algorithms.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use moonly::{
    post::{
        calc_gauss_dual_wave, evaluate_lens_flare_ghost, karis_box4_average, karis_downsample_13,
    },
    prelude::*,
    shadows::LocalShadowBatches,
};

fn build_test_reflectors(count: usize) -> Vec<GiReflectorBox> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let angle = (i as f32) * 0.42;
        let radius = 3.0 + (i as f32) * 1.1;
        let center = Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius);
        out.push(GiReflectorBox::from_center_extents(
            center,
            Vec3::new(1.2, 1.5, 1.2),
            Vec3::new(0.75, 0.55, 0.40),
        ));
    }
    out
}

fn build_test_lights(count: usize) -> Vec<MoonlyLight> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let offset = (i as f32) * 2.5;
        out.push(MoonlyLight::spot(
            Vec3::new(offset * 0.2, 3.5, 0.0),
            Vec3::NEG_Y,
            Vec3::new(3.5, 2.8, 2.1),
            10.0,
            1.25,
        ));
    }
    out
}

fn bench_smap_allocator(c: &mut Criterion) {
    let tile_sizes: [u32; 36] = [
        1024, 512, 512, 512, 256, 256, 256, 256, 512, 256, 128, 128, 128, 128, 256, 256, 128, 128,
        64, 64, 64, 64, 128, 128, 64, 64, 64, 64, 256, 128, 64, 64, 64, 64, 64, 64,
    ];
    let mut group = c.benchmark_group("smap_allocator");
    let mut alloc = SmapAllocator::new(4096);

    group.bench_function("monolith_critical_point_36_tiles", |b| {
        b.iter(|| {
            alloc.initialize(4096);
            let mut packed = 0_u32;
            for &sz in black_box(&tile_sizes) {
                if alloc.push_monolith(sz).is_some() {
                    packed += 1;
                }
            }
            black_box(packed)
        });
    });

    group.bench_function("moonly_enhanced_bssf_36_tiles", |b| {
        b.iter(|| {
            alloc.initialize(4096);
            let mut packed = 0_u32;
            for &sz in black_box(&tile_sizes) {
                if alloc.push_enhanced_bssf(sz).is_some() {
                    packed += 1;
                }
            }
            black_box(packed)
        });
    });
    group.finish();
}

fn bench_occ_rasterizer(c: &mut Criterion) {
    let mut occ = OccRasterizer::default();
    occ.clear();
    occ.rasterize_triangle(
        Vec3::new(6.0, 6.0, 0.22),
        Vec3::new(56.0, 6.0, 0.22),
        Vec3::new(6.0, 56.0, 0.22),
    );
    occ.rasterize_triangle(
        Vec3::new(56.0, 6.0, 0.22),
        Vec3::new(56.0, 56.0, 0.22),
        Vec3::new(6.0, 56.0, 0.22),
    );
    occ.propagate();

    let mut rects = Vec::with_capacity(64);
    for i in 0..64_usize {
        let x0 = (i * 3) % 44;
        let y0 = (i * 5) % 44;
        let depth = if i % 4 == 0 { 0.12 } else { 0.65 };
        rects.push(OccTestRect {
            x0,
            y0,
            x1: x0 + 14,
            y1: y0 + 14,
            depth,
        });
    }

    let mut group = c.benchmark_group("occ_rasterizer_64x64");
    group.bench_function("monolith_recursive_64_queries", |b| {
        b.iter(|| {
            let mut visible = 0_i32;
            for &r in black_box(&rects) {
                if occ.test_monolith(r) > 0 {
                    visible += 1;
                }
            }
            black_box(visible)
        });
    });

    group.bench_function("moonly_enhanced_fast_64_queries", |b| {
        b.iter(|| {
            let mut visible = 0_i32;
            for &r in black_box(&rects) {
                if occ.test_enhanced_fast(r) > 0 {
                    visible += 1;
                }
            }
            black_box(visible)
        });
    });

    group.bench_function("rasterize_16_quads_and_propagate", |b| {
        let mut rast = OccRasterizer::default();
        b.iter(|| {
            rast.clear();
            for i in 0..16_usize {
                let x = (i % 4) as f32 * 14.0 + 2.0;
                let y = (i / 4) as f32 * 14.0 + 2.0;
                let z = 0.15 + (i as f32) * 0.03;
                rast.rasterize_triangle(
                    Vec3::new(x, y, z),
                    Vec3::new(x + 12.0, y, z),
                    Vec3::new(x, y + 12.0, z),
                );
                rast.rasterize_triangle(
                    Vec3::new(x + 12.0, y, z),
                    Vec3::new(x + 12.0, y + 12.0, z),
                    Vec3::new(x, y + 12.0, z),
                );
            }
            rast.propagate();
            black_box(rast.buf_depth_0.first().copied())
        });
    });
    group.finish();
}

fn bench_gi_radiosity(c: &mut Criterion) {
    let reflectors = build_test_reflectors(64);
    let lights = build_test_lights(12);
    let mut out_virtual = Vec::with_capacity(768);
    let mut scratch = Vec::with_capacity(64);

    for photons in [16, 32, 64] {
        let cfg = GiRadiosityConfig {
            num_photons: photons,
            cosine_weighted: true,
            ..GiRadiosityConfig::default()
        };
        let mut group = c.benchmark_group(format!("gi_radiosity_{photons}_photons"));
        group.bench_function("monolith_bruteforce_12_lights_64_boxes", |b| {
            b.iter(|| {
                out_virtual.clear();
                for light in black_box(&lights) {
                    gi_generate_monolith(
                        light,
                        &cfg,
                        black_box(reflectors.as_slice()),
                        &mut out_virtual,
                    );
                }
                black_box(out_virtual.len())
            });
        });

        group.bench_function("moonly_enhanced_broadphase_12_lights_64_boxes", |b| {
            b.iter(|| {
                out_virtual.clear();
                for light in black_box(&lights) {
                    gi_generate_enhanced(
                        light,
                        &cfg,
                        black_box(&reflectors),
                        &mut out_virtual,
                        &mut scratch,
                    );
                }
                black_box(out_virtual.len())
            });
        });
        group.finish();
    }
}

fn bench_cascades_and_post(c: &mut Criterion) {
    let cam = CameraFrustumParams::default();
    let cascade_cfg = SunCascadeConfig::default();
    let sun_dir = Vec3::new(-0.45, -0.78, -0.43).normalize();
    let lights = build_test_lights(24);
    let mut cascades = SunCascades::default();

    let mut group = c.benchmark_group("cascades_and_hdr10");
    group.bench_function("sun_3_cascades_plus_24_spot_xforms", |b| {
        b.iter(|| {
            cascades.compute(black_box(&cam), sun_dir, &cascade_cfg, 2048);
            let mut sum_size = 0_u32;
            for light in black_box(&lights) {
                let xf = SpotShadowXform::compute_xf_spot(light, Vec3::ZERO, 2048, 256);
                sum_size += xf.smap_size;
            }
            black_box((cascades.cascades.len(), sum_size))
        });
    });

    group.bench_function("hdr10_aces_agx_colored_ao_256_samples", |b| {
        b.iter(|| {
            let mut acc = Vec3::ZERO;
            for i in 0..256_u32 {
                let v = (i as f32) * 0.02;
                let hdr = Vec3::new(v * 1.6, v * 1.1, v * 0.7);
                let ao = compute_colored_ao_cpu(0.65, Vec3::new(0.8, 0.5, 0.3));
                let k = karis_box4_average(hdr, hdr * 0.5, ao, Vec3::ONE);
                let aces = evaluate_tonemap_cpu(hdr + k, TonemapOperator::AcesHill);
                let agx = evaluate_tonemap_cpu(aces, TonemapOperator::AgX);
                acc += encode_pq_st2084(agx, 1000.0);
            }
            black_box(acc)
        });
    });

    group.bench_function("local_shadow_batches_24_lights", |b| {
        let mut pkg = LightPackage::default();
        pkg.v_shadowed.clone_from(&lights);
        let cfg = MoonlyConfig::default();
        let mut batches = LocalShadowBatches::default();
        b.iter(|| {
            batches.rebuild(black_box(&pkg), black_box(&cfg), Vec3::ZERO);
            black_box((batches.batch_count(), batches.view_count()))
        });
    });

    group.bench_function("bloom_gauss_and_lens_flare_256_samples", |b| {
        let samples = [Vec3::new(1.4, 0.9, 0.5); 13];
        b.iter(|| {
            let weights = calc_gauss_dual_wave(black_box(0.35));
            let mut acc = Vec3::splat(weights.first().copied().unwrap_or(0.0));
            for i in 0..256_u32 {
                let u = ((i & 15) as f32) * (1.0 / 16.0);
                let v = ((i >> 4) as f32) * (1.0 / 16.0);
                let ds = karis_downsample_13(black_box(samples));
                let flare =
                    evaluate_lens_flare_ghost(Vec2::new(u, v), Vec2::splat(0.3), 0.18, 0.42);
                acc += ds + flare;
            }
            black_box(acc)
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_smap_allocator,
    bench_occ_rasterizer,
    bench_gi_radiosity,
    bench_cascades_and_post
);
criterion_main!(benches);

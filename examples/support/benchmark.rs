use std::{path::PathBuf, time::Instant};

use bevy::{
    app::AppExit, ecs::system::SystemParam, prelude::*, render::renderer::RenderAdapterInfo,
};
use moonly::prelude::*;

use super::reference::BenchmarkScenario;

#[derive(Resource, Debug, Clone)]
pub(crate) struct BenchmarkOptions {
    pub(crate) scenario_name: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) warmup_frames: u32,
    pub(crate) measured_frames: u32,
    pub(crate) output: PathBuf,
    pub(crate) tier: MoonlyPipelineTier,
    pub(crate) renderer: String,
}

impl BenchmarkOptions {
    pub(crate) fn from_env_args() -> Self {
        let scenario_name = string_arg("--scenario", "many_lights_64");
        let renderer = string_arg("--renderer", "legacy_forward_analytic");
        let tier = match string_arg("--tier", "monolith_r4").as_str() {
            "moonly_enhanced" => MoonlyPipelineTier::MoonlyEnhanced,
            _ => MoonlyPipelineTier::MonolithR4,
        };
        Self {
            output: PathBuf::from(string_arg(
                "--output",
                &format!(
                    "target/benchmarks/{renderer}-{scenario_name}-{}x{}.json",
                    numeric_arg("--width", 1280),
                    numeric_arg("--height", 720)
                ),
            )),
            scenario_name,
            width: numeric_arg("--width", 1280),
            height: numeric_arg("--height", 720),
            warmup_frames: numeric_arg("--warmup", 120),
            measured_frames: numeric_arg("--frames", 600).max(1),
            tier,
            renderer,
        }
    }

    pub(crate) const fn total_frames(&self) -> u32 {
        self.warmup_frames + self.measured_frames
    }
}

fn string_arg(name: &str, default: &str) -> String {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn numeric_arg(name: &str, default: u32) -> u32 {
    string_arg(name, &default.to_string())
        .parse()
        .unwrap_or(default)
}

#[derive(Resource)]
pub(crate) struct BenchmarkState {
    frame: u32,
    frame_started: Option<Instant>,
    cpu_frame_ms: Vec<f64>,
}

impl BenchmarkState {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            frame: 0,
            frame_started: None,
            cpu_frame_ms: Vec::with_capacity(capacity),
        }
    }
}

pub(crate) fn begin_cpu_frame(mut state: ResMut<BenchmarkState>) {
    state.frame_started = Some(Instant::now());
}

#[derive(SystemParam)]
pub(crate) struct BenchmarkReportInputs<'w> {
    options: Res<'w, BenchmarkOptions>,
    state: ResMut<'w, BenchmarkState>,
    adapter: Res<'w, RenderAdapterInfo>,
    package: Res<'w, LightPackage>,
    diagnostics: Res<'w, MoonlyGpuDiagnostics>,
}

pub(crate) fn end_cpu_frame(inputs: BenchmarkReportInputs, mut exit: MessageWriter<AppExit>) {
    let BenchmarkReportInputs {
        options,
        mut state,
        adapter,
        package,
        diagnostics,
    } = inputs;
    state.frame += 1;
    if state.frame > options.warmup_frames
        && let Some(started) = state.frame_started.take()
    {
        state
            .cpu_frame_ms
            .push(started.elapsed().as_secs_f64() * 1000.0);
    }
    if state.frame == options.total_frames() {
        match save_report(&options, &state, &adapter, &package, &diagnostics) {
            Ok(()) => {
                exit.write(AppExit::Success);
            }
            Err(error) => {
                error!("Benchmark report failed: {error}");
                exit.write(AppExit::error());
            }
        }
    }
}

fn save_report(
    options: &BenchmarkOptions,
    state: &BenchmarkState,
    adapter: &RenderAdapterInfo,
    package: &LightPackage,
    diagnostics: &MoonlyGpuDiagnostics,
) -> Result<(), String> {
    if state.cpu_frame_ms.len() != options.measured_frames as usize {
        return Err("measured frame count does not match the request".to_string());
    }
    let mut sorted = state.cpu_frame_ms.clone();
    sorted.sort_by(f64::total_cmp);
    let summary = FrameSummary {
        p50: percentile(&sorted, 50),
        p95: percentile(&sorted, 95),
        gpu_json: gpu_timings_json(diagnostics),
    };
    let report = report_json(options, state, adapter, package, &summary);
    if let Some(parent) = options.output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&options.output, report).map_err(|error| error.to_string())?;
    info!(
        "Benchmark {}: CPU p50 {:.3} ms, p95 {:.3} ms; report {}",
        options.scenario_name,
        summary.p50,
        summary.p95,
        options.output.display()
    );
    Ok(())
}

fn percentile(sorted: &[f64], percentile: usize) -> f64 {
    let index = (sorted.len().saturating_sub(1)) * percentile / 100;
    sorted.get(index).copied().unwrap_or_default()
}

struct FrameSummary {
    p50: f64,
    p95: f64,
    gpu_json: String,
}

fn report_json(
    options: &BenchmarkOptions,
    state: &BenchmarkState,
    adapter: &RenderAdapterInfo,
    package: &LightPackage,
    summary: &FrameSummary,
) -> String {
    let samples = state
        .cpu_frame_ms
        .iter()
        .map(|sample| format!("{sample:.6}"))
        .collect::<Vec<_>>()
        .join(",");
    let tier = match options.tier {
        MoonlyPipelineTier::MoonlyEnhanced => "MoonlyEnhanced",
        _ => "MonolithR4",
    };
    format!(
        concat!(
            "{{\n",
            "  \"schema\": 1,\n",
            "  \"profile\": \"dev\",\n",
            "  \"renderer\": \"{}\",\n",
            "  \"scenario\": \"{}\",\n",
            "  \"tier\": \"{}\",\n",
            "  \"resolution\": [{}, {}],\n",
            "  \"adapter\": \"{}\",\n",
            "  \"backend\": \"{:?}\",\n",
            "  \"warmup_frames\": {},\n",
            "  \"measured_frames\": {},\n",
            "  \"light_package_size\": {},\n",
            "  \"gbuffer_target_bytes\": {},\n",
            "  \"cpu_frame_ms\": {{\"p50\": {:.6}, \"p95\": {:.6}}},\n",
            "  \"cpu_frame_samples_ms\": [{}],\n",
            "  \"gpu_timings\": {}\n",
            "}}\n"
        ),
        json_escape(&options.renderer),
        json_escape(&options.scenario_name),
        tier,
        options.width,
        options.height,
        json_escape(&adapter.name),
        adapter.backend,
        options.warmup_frames,
        options.measured_frames,
        package.total_count(),
        u64::from(options.width) * u64::from(options.height) * 36,
        summary.p50,
        summary.p95,
        samples,
        summary.gpu_json,
    )
}

fn gpu_timings_json(diagnostics: &MoonlyGpuDiagnostics) -> String {
    let mut available = false;
    let mut entries = Vec::new();
    for pass in MoonlyPass::ALL {
        let value = match diagnostics.timing(pass) {
            GpuTiming::Available(duration) => {
                available = true;
                format!("{:.6}", duration.as_secs_f64() * 1000.0)
            }
            GpuTiming::Pending => "\"pending\"".to_string(),
            GpuTiming::Unavailable => "\"unavailable\"".to_string(),
        };
        entries.push(format!("\"{}\":{}", pass.label(), value));
    }
    if available {
        format!(
            "{{\"status\":\"available\",\"passes_ms\":{{{}}}}}",
            entries.join(",")
        )
    } else {
        "{\"status\":\"unavailable\"}".to_string()
    }
}

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn scenario_config(tier: MoonlyPipelineTier) -> MoonlyConfig {
    match tier {
        MoonlyPipelineTier::MoonlyEnhanced => MoonlyConfig::enhanced(),
        _ => MoonlyConfig::moonly_r4(),
    }
}

pub(crate) fn spawn_scenario_lights(commands: &mut Commands, scenario: BenchmarkScenario) {
    for index in 0..scenario.local_lights {
        let angle = index as f32 * 2.399_963;
        let radius = 1.5 + (index % 16) as f32 * 0.35;
        let position = Vec3::new(angle.cos() * radius, 2.2, angle.sin() * radius);
        let flags = if scenario.shadowed {
            MoonlyLightFlags::SHADOW.union(MoonlyLightFlags::GI_ENABLED)
        } else {
            MoonlyLightFlags::default()
        };
        commands.spawn((
            MoonlyLight {
                light_type: if scenario.shadowed {
                    MoonlyLightType::Spot
                } else {
                    MoonlyLightType::Point
                },
                flags,
                color: Vec3::new(1.4, 1.1, 0.8),
                position,
                range: 10.0,
                ..MoonlyLight::default()
            },
            Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y),
        ));
    }
}

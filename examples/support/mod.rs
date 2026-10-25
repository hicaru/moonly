pub(crate) mod benchmark;
pub(crate) mod capture;
pub(crate) mod native_reference;
pub(crate) mod reference;
pub(crate) mod reference_scene;
pub(crate) mod scene;

use std::time::Duration;

use bevy::{
    prelude::*,
    time::TimeUpdateStrategy,
    window::{Window, WindowPlugin, WindowResolution},
};

#[allow(unused_imports)]
pub(crate) use capture::{CaptureOptions, CaptureState, capture_system};

pub(crate) const CAPTURE_WIDTH: u32 = 1280;
pub(crate) const CAPTURE_HEIGHT: u32 = 720;

pub(crate) fn numeric_arg(name: &str, default: u32) -> u32 {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

pub(crate) fn float_arg(name: &str, default: f32) -> f32 {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

pub(crate) fn window_plugin(title: &str) -> WindowPlugin {
    sized_window_plugin(title, CAPTURE_WIDTH, CAPTURE_HEIGHT)
}

pub(crate) fn sized_window_plugin(title: &str, width: u32, height: u32) -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: title.to_string(),
            resolution: WindowResolution::new(width, height).with_scale_factor_override(1.0),
            ..Window::default()
        }),
        ..WindowPlugin::default()
    }
}

pub(crate) fn deterministic_time() -> TimeUpdateStrategy {
    TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0))
}

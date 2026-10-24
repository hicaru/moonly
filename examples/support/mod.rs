pub(crate) mod scene;

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    time::TimeUpdateStrategy,
    window::{Window, WindowPlugin, WindowResolution},
};

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

#[derive(Resource, Debug, Clone)]
pub(crate) struct CaptureOptions {
    pub(crate) screenshot_frame: Option<u32>,
    pub(crate) screenshot_path: PathBuf,
    pub(crate) exit_frame: Option<u32>,
}

impl CaptureOptions {
    pub(crate) fn from_env_args(default_path: &str) -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut options = Self {
            screenshot_frame: None,
            screenshot_path: PathBuf::from(default_path),
            exit_frame: None,
        };
        let mut index = 1_usize;
        while let Some(arg) = args.get(index) {
            match arg.as_str() {
                "--screenshot-after" => {
                    options.screenshot_frame = args.get(index + 1).and_then(|v| v.parse().ok());
                    if let Some(path) = args.get(index + 2) {
                        options.screenshot_path = PathBuf::from(path);
                    }
                    index += 3;
                }
                "--exit-after" => {
                    options.exit_frame = args.get(index + 1).and_then(|v| v.parse().ok());
                    index += 2;
                }
                _ => index += 1,
            }
        }
        options
    }
}

#[derive(Resource, Default)]
pub(crate) struct CaptureState {
    finished: bool,
    failed: bool,
}

pub(crate) fn window_plugin(title: &str) -> WindowPlugin {
    WindowPlugin {
        primary_window: Some(Window {
            title: title.to_string(),
            resolution: WindowResolution::new(CAPTURE_WIDTH, CAPTURE_HEIGHT),
            ..Window::default()
        }),
        ..WindowPlugin::default()
    }
}

pub(crate) fn deterministic_time() -> TimeUpdateStrategy {
    TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0))
}

pub(crate) fn capture_system(
    mut commands: Commands,
    options: Res<CaptureOptions>,
    state: Res<CaptureState>,
    mut exit: MessageWriter<AppExit>,
    mut frame: Local<u32>,
) {
    *frame += 1;
    if state.failed {
        exit.write(AppExit::error());
        return;
    }
    if options.screenshot_frame == Some(*frame) {
        let path = options.screenshot_path.clone();
        commands.spawn(Screenshot::primary_window()).observe(
            move |captured: On<ScreenshotCaptured>, mut state: ResMut<CaptureState>| {
                match save_and_validate(&captured.image, &path) {
                    Ok(report) => {
                        info!(
                            "Validated {} ({}x{}, range {})",
                            path.display(),
                            report.width,
                            report.height,
                            report.channel_range
                        );
                        state.finished = true;
                    }
                    Err(error) => {
                        error!("Screenshot validation failed: {error}");
                        state.failed = true;
                    }
                }
            },
        );
    }
    if options.exit_frame.is_some_and(|limit| *frame >= limit) {
        if options.screenshot_frame.is_none() || state.finished {
            exit.write(AppExit::Success);
        } else {
            error!("Screenshot did not finish before --exit-after");
            exit.write(AppExit::error());
        }
    }
}

struct CaptureReport {
    width: u32,
    height: u32,
    channel_range: u8,
}

fn save_and_validate(image: &Image, path: &Path) -> Result<CaptureReport, String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let expected = (
        image.texture_descriptor.size.width,
        image.texture_descriptor.size.height,
    );
    let dynamic = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| error.to_string())?;
    encode_screenshot(dynamic)
        .save(path)
        .map_err(|error| error.to_string())?;
    validate_png(path, expected)
}

fn encode_screenshot(dynamic: image::DynamicImage) -> image::DynamicImage {
    if !matches!(
        dynamic,
        image::DynamicImage::ImageRgb32F(_) | image::DynamicImage::ImageRgba32F(_)
    ) {
        return image::DynamicImage::ImageRgb8(dynamic.to_rgb8());
    }
    let rgb = dynamic.into_rgb32f();
    let encoded = image::RgbImage::from_fn(rgb.width(), rgb.height(), |x, y| {
        let [r, g, b] = rgb.get_pixel(x, y).0;
        let color = Srgba::from(LinearRgba::rgb(r, g, b));
        image::Rgb(color.to_u8_array_no_alpha())
    });
    image::DynamicImage::ImageRgb8(encoded)
}

fn validate_png(path: &Path, expected: (u32, u32)) -> Result<CaptureReport, String> {
    let decoded = image::open(path)
        .map_err(|error| error.to_string())?
        .to_rgb8();
    let (width, height) = decoded.dimensions();
    if (width, height) != expected {
        return Err(format!(
            "unexpected dimensions {width}x{height}, expected {}x{}",
            expected.0, expected.1
        ));
    }
    let Some(first) = decoded.pixels().next() else {
        return Err("empty image".to_string());
    };
    let (min, max) = decoded
        .pixels()
        .fold((u8::MAX, u8::MIN), |(min, max), pixel| {
            let pixel_min = pixel.0.iter().copied().min().unwrap_or(min);
            let pixel_max = pixel.0.iter().copied().max().unwrap_or(max);
            (min.min(pixel_min), max.max(pixel_max))
        });
    if max.saturating_sub(min) < 8 {
        return Err(format!("image is nearly uniform: range {}", max - min));
    }
    if decoded.pixels().all(|pixel| pixel == first) {
        return Err("image has only one color".to_string());
    }
    Ok(CaptureReport {
        width,
        height,
        channel_range: max - min,
    })
}

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bevy::{
    app::AppExit,
    input::keyboard::KeyCode,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        render_resource::PipelineCache,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};

const CAPTURE_PATH: &str = "MOONLY_CAPTURE_PATH";
const CAPTURE_FRAME: &str = "MOONLY_CAPTURE_FRAME";
const EXIT_FRAME: &str = "MOONLY_EXIT_FRAME";
const INPUT_EVENTS: &str = "MOONLY_INPUT_EVENTS";
const DEFAULT_CAPTURE_FRAME: u32 = 90;
const DEFAULT_EXIT_FRAME: u32 = 110;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyAction {
    Pulse,
    Press,
    Release,
}

#[derive(Debug, Clone, Copy)]
struct TimedKey {
    frame: u32,
    key: KeyCode,
    action: KeyAction,
}

#[derive(Resource, Debug, Clone)]
pub(crate) struct CaptureOptions {
    path: Option<PathBuf>,
    capture_frame: u32,
    exit_frame: u32,
    keys: Box<[TimedKey]>,
}

impl CaptureOptions {
    pub(crate) fn from_env() -> Option<Self> {
        let configured = [CAPTURE_PATH, CAPTURE_FRAME, EXIT_FRAME, INPUT_EVENTS]
            .iter()
            .any(|name| std::env::var_os(name).is_some());
        if !configured {
            return None;
        }
        let path = std::env::var_os(CAPTURE_PATH).map(PathBuf::from);
        let capture_frame = env_number(CAPTURE_FRAME, DEFAULT_CAPTURE_FRAME);
        let exit_frame = env_number(EXIT_FRAME, DEFAULT_EXIT_FRAME).max(capture_frame + 1);
        let events = std::env::var(INPUT_EVENTS).unwrap_or_default();
        Some(Self {
            path,
            capture_frame,
            exit_frame,
            keys: parse_input_events(&events),
        })
    }
}

pub(crate) fn configure(app: &mut App) {
    let Some(options) = CaptureOptions::from_env() else {
        return;
    };
    let readiness = CaptureReadiness::default();
    app.insert_resource(options)
        .insert_resource(readiness.clone())
        .init_resource::<CaptureState>()
        .add_systems(
            PreUpdate,
            inject_capture_input.after(bevy::input::InputSystems),
        )
        .add_systems(Last, capture_frame);
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.insert_resource(readiness).add_systems(
            Render,
            update_capture_readiness.in_set(RenderSystems::Render),
        );
    }
}

#[derive(Resource, Clone, Default)]
pub(crate) struct CaptureReadiness(Arc<AtomicBool>);

fn update_capture_readiness(readiness: Res<CaptureReadiness>, cache: Res<PipelineCache>) {
    let ready = cache.waiting_pipelines().next().is_none();
    readiness.0.store(ready, Ordering::Relaxed);
}

#[derive(Resource, Default)]
pub(crate) struct CaptureState {
    clock: u32,
    requested: bool,
    finished: bool,
    failed: bool,
}

fn env_number(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn parse_input_events(events: &str) -> Box<[TimedKey]> {
    events
        .split(',')
        .filter_map(|event| {
            let (frame, key) = event.split_once(':')?;
            let key = key.trim();
            let (action, key) = if let Some(key) = key.strip_prefix('+') {
                (KeyAction::Press, key)
            } else if let Some(key) = key.strip_prefix('-') {
                (KeyAction::Release, key)
            } else {
                (KeyAction::Pulse, key)
            };
            Some(TimedKey {
                frame: frame.trim().parse().ok()?,
                key: parse_key(key)?,
                action,
            })
        })
        .collect()
}

fn parse_key(key: &str) -> Option<KeyCode> {
    match key {
        "Space" => Some(KeyCode::Space),
        "Enter" => Some(KeyCode::Enter),
        "ArrowUp" => Some(KeyCode::ArrowUp),
        "ArrowDown" => Some(KeyCode::ArrowDown),
        "ArrowLeft" => Some(KeyCode::ArrowLeft),
        "ArrowRight" => Some(KeyCode::ArrowRight),
        "1" => Some(KeyCode::Digit1),
        "2" => Some(KeyCode::Digit2),
        "3" => Some(KeyCode::Digit3),
        "4" => Some(KeyCode::Digit4),
        "5" => Some(KeyCode::Digit5),
        "6" => Some(KeyCode::Digit6),
        "7" => Some(KeyCode::Digit7),
        "8" => Some(KeyCode::Digit8),
        "9" => Some(KeyCode::Digit9),
        "A" => Some(KeyCode::KeyA),
        "D" => Some(KeyCode::KeyD),
        "E" => Some(KeyCode::KeyE),
        "H" => Some(KeyCode::KeyH),
        "L" => Some(KeyCode::KeyL),
        "O" => Some(KeyCode::KeyO),
        "P" => Some(KeyCode::KeyP),
        "Q" => Some(KeyCode::KeyQ),
        "R" => Some(KeyCode::KeyR),
        "S" => Some(KeyCode::KeyS),
        "W" => Some(KeyCode::KeyW),
        _ => None,
    }
}

pub(crate) fn inject_capture_input(
    options: Option<Res<CaptureOptions>>,
    mut state: ResMut<CaptureState>,
    keyboard: Option<ResMut<ButtonInput<KeyCode>>>,
) {
    let (Some(options), Some(mut keyboard)) = (options, keyboard) else {
        return;
    };
    state.clock += 1;
    let clock = state.clock;
    debug!(clock, "Moonly compatibility capture frame");
    for event in options.keys.iter().filter(|event| {
        event.action == KeyAction::Pulse && event.frame.checked_add(1) == Some(clock)
    }) {
        keyboard.release(event.key);
    }
    for event in options.keys.iter().filter(|event| event.frame == clock) {
        match event.action {
            KeyAction::Pulse | KeyAction::Press => keyboard.press(event.key),
            KeyAction::Release => keyboard.release(event.key),
        }
    }
}

pub(crate) fn capture_frame(
    mut commands: Commands,
    options: Option<Res<CaptureOptions>>,
    readiness: Option<Res<CaptureReadiness>>,
    mut state: ResMut<CaptureState>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(options) = options else {
        return;
    };
    if state.failed {
        exit.write(AppExit::error());
        return;
    }
    let ready = readiness.is_some_and(|ready| ready.0.load(Ordering::Relaxed));
    if options.path.is_some() && ready && state.clock >= options.capture_frame && !state.requested {
        state.requested = true;
        let path = options.path.clone();
        commands.spawn(Screenshot::primary_window()).observe(
            move |captured: On<ScreenshotCaptured>, mut state: ResMut<CaptureState>| {
                let Some(path) = path.as_ref() else {
                    state.failed = true;
                    return;
                };
                match save_screenshot(&captured.image, path) {
                    Ok(()) => state.finished = true,
                    Err(error) => {
                        error!("Moonly capture failed: {error}");
                        state.failed = true;
                    }
                }
            },
        );
    }
    let capture_complete = options.path.is_none() || state.finished;
    if state.clock >= options.exit_frame && capture_complete {
        exit.write(AppExit::Success);
    }
}

fn save_screenshot(image: &Image, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let dynamic = image
        .clone()
        .try_into_dynamic()
        .map_err(|error| error.to_string())?;
    let rgb = dynamic.to_rgb8();
    let (min, max) = rgb.pixels().fold((u8::MAX, u8::MIN), |(min, max), pixel| {
        let low = pixel.0.iter().copied().min().unwrap_or(min);
        let high = pixel.0.iter().copied().max().unwrap_or(max);
        (min.min(low), max.max(high))
    });
    if max.saturating_sub(min) < 8 {
        return Err(format!("image is nearly uniform: range {}", max - min));
    }
    rgb.save(path).map_err(|error| error.to_string())?;
    info!("Saved Moonly capture {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_events_accept_scene_controls() {
        let events = parse_input_events("4:1, 8:Space,12:P,16:ArrowLeft");
        assert_eq!(events.len(), 4);
        assert!(events.iter().any(|event| event.key == KeyCode::Digit1));
        assert!(events.iter().any(|event| event.key == KeyCode::Space));
        assert!(events.iter().any(|event| event.key == KeyCode::KeyP));
        assert!(events.iter().any(|event| event.key == KeyCode::ArrowLeft));
    }

    #[test]
    fn input_events_support_held_keys() {
        let events = parse_input_events("4:+Q,20:-Q");
        assert_eq!(
            events.first().map(|event| event.action),
            Some(KeyAction::Press)
        );
        assert_eq!(
            events.get(1).map(|event| event.action),
            Some(KeyAction::Release)
        );
    }

    #[test]
    fn invalid_input_events_are_ignored() {
        assert!(parse_input_events("bad,4:unknown,7").is_empty());
    }
}

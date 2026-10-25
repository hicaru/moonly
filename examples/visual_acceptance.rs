//! Deterministic GPU captures, pixel probes, and image comparisons.

#[allow(dead_code)]
mod support;

use std::path::{Path, PathBuf};

use bevy::app::AppExit;
use support::reference::{compare_png, pixel_matches};

struct PixelProbe {
    x: u32,
    y: u32,
    expected: [u8; 3],
}

struct Scene {
    name: &'static str,
    argument: Option<&'static str>,
    reference: &'static str,
    tolerance: f64,
    probe: PixelProbe,
}

const SCENES: &[Scene] = &[
    Scene {
        name: "single_spot",
        argument: None,
        reference: "reference/single-spot.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 650,
            y: 300,
            expected: [249, 175, 89],
        },
    },
    Scene {
        name: "native_spot",
        argument: Some("--native-light"),
        reference: "reference/native-light.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 650,
            y: 300,
            expected: [202, 106, 43],
        },
    },
    Scene {
        name: "glow",
        argument: Some("--glow"),
        reference: "reference/glow.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 650,
            y: 300,
            expected: [255, 215, 132],
        },
    },
    Scene {
        name: "point_shadow",
        argument: Some("--point-shadow"),
        reference: "reference/point-shadow.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 650,
            y: 300,
            expected: [199, 99, 34],
        },
    },
    Scene {
        name: "alpha_caster",
        argument: Some("--alpha-caster"),
        reference: "reference/alpha-caster.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 550,
            y: 300,
            expected: [222, 129, 56],
        },
    },
    Scene {
        name: "transparent",
        argument: Some("--transparent"),
        reference: "reference/transparent.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 400,
            y: 400,
            expected: [93, 154, 193],
        },
    },
    Scene {
        name: "fog_transparent",
        argument: Some("--fog-transparent"),
        reference: "reference/fog-transparent.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 400,
            y: 400,
            expected: [112, 159, 193],
        },
    },
    Scene {
        name: "transmissive",
        argument: Some("--transmissive"),
        reference: "reference/transmissive.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 400,
            y: 400,
            expected: [89, 112, 104],
        },
    },
    Scene {
        name: "sun_cascades",
        argument: Some("--sun"),
        reference: "reference/sun-cascades.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 800,
            y: 550,
            expected: [154, 157, 124],
        },
    },
    Scene {
        name: "cascade_motion",
        argument: Some("--cascade-motion"),
        reference: "reference/cascade-motion.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 650,
            y: 300,
            expected: [247, 173, 92],
        },
    },
    Scene {
        name: "moving_casters",
        argument: Some("--moving-caster"),
        reference: "reference/moving-caster.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 650,
            y: 300,
            expected: [249, 173, 88],
        },
    },
    Scene {
        name: "screen_shafts",
        argument: Some("--screen-shafts"),
        reference: "reference/screen-shafts.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 640,
            y: 60,
            expected: [50, 46, 38],
        },
    },
    Scene {
        name: "overlay",
        argument: Some("--overlay"),
        reference: "reference/overlay.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 490,
            y: 375,
            expected: [220, 143, 61],
        },
    },
    Scene {
        name: "fog",
        argument: Some("--fog"),
        reference: "reference/fog.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 800,
            y: 550,
            expected: [123, 128, 110],
        },
    },
    Scene {
        name: "froxel",
        argument: Some("--froxel"),
        reference: "reference/froxel.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 800,
            y: 550,
            expected: [159, 161, 134],
        },
    },
    Scene {
        name: "wet_environment",
        argument: Some("--wet"),
        reference: "reference/wet-environment.png",
        tolerance: 1.0,
        probe: PixelProbe {
            x: 800,
            y: 550,
            expected: [79, 80, 53],
        },
    },
];

fn main() -> AppExit {
    for scene in SCENES {
        if let Err(error) = validate_scene(scene) {
            eprintln!("{}: {error}", scene.name);
            return AppExit::error();
        }
    }
    AppExit::Success
}

fn validate_scene(scene: &Scene) -> Result<(), String> {
    let candidate = PathBuf::from(format!("target/visual-audit/acceptance-{}.png", scene.name));
    let diff = PathBuf::from(format!("target/visual-audit/diff-{}.png", scene.name));
    run_capture(scene, &candidate)?;
    let report = compare_png(
        Path::new(scene.reference),
        &candidate,
        &diff,
        scene.tolerance,
    )?;
    if !report.matches {
        return Err(format!(
            "MAE {:.6}, RMSE {:.6}, max {}; diff {}",
            report.mae,
            report.rmse,
            report.max_error,
            diff.display()
        ));
    }
    if !pixel_matches(
        &candidate,
        scene.probe.x,
        scene.probe.y,
        scene.probe.expected,
        2,
    )? {
        return Err(format!(
            "pixel ({}, {}) does not match {:?}",
            scene.probe.x, scene.probe.y, scene.probe.expected
        ));
    }
    println!(
        "{}: MATCH MAE {:.6}, RMSE {:.6}, max {}",
        scene.name, report.mae, report.rmse, report.max_error
    );
    Ok(())
}

fn run_capture(scene: &Scene, candidate: &Path) -> Result<(), String> {
    let mut command = std::process::Command::new("cargo");
    command.args([
        "run",
        "--example",
        "moonly_reference_lab",
        "--",
        "--screenshot-after",
        "45",
    ]);
    command.arg(candidate);
    command.args(["--offscreen", "--exit-after", "60"]);
    if let Some(argument) = scene.argument {
        command.arg(argument);
    }
    command.env("RUST_LOG", "error");
    let status = command.status().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("capture failed with status {status}"))
    }
}

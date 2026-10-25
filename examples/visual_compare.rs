//! Compare two PNG images without changing reference files.

#[allow(dead_code)]
mod support;

use std::path::PathBuf;

use bevy::app::AppExit;
use support::reference::compare_png;

fn main() -> AppExit {
    let args: Vec<String> = std::env::args().collect();
    let Some(reference) = args.get(1) else {
        return AppExit::error();
    };
    let Some(candidate) = args.get(2) else {
        return AppExit::error();
    };
    let diff = args.get(3).map_or_else(
        || PathBuf::from("target/visual-audit/diff.png"),
        PathBuf::from,
    );
    let tolerance = args
        .get(4)
        .and_then(|value| value.parse().ok())
        .unwrap_or(1.0);
    match compare_png(
        std::path::Path::new(reference),
        std::path::Path::new(candidate),
        &diff,
        tolerance,
    ) {
        Ok(report) if report.matches => {
            println!(
                "MATCH mae {:.6}, rmse {:.6}, max {}",
                report.mae, report.rmse, report.max_error
            );
            AppExit::Success
        }
        Ok(report) => {
            eprintln!(
                "DIFF mae {:.6}, rmse {:.6}, max {}; image {}",
                report.mae,
                report.rmse,
                report.max_error,
                diff.display()
            );
            AppExit::error()
        }
        Err(error) => {
            eprintln!("Image comparison failed: {error}");
            AppExit::error()
        }
    }
}

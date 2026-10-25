use bevy::prelude::*;

pub(crate) const MOONLY_REFERENCE_COMMIT: &str = "c57fe5b9";

#[derive(Resource, Debug, Clone, Copy)]
pub(crate) struct BenchmarkScenario {
    pub(crate) name: &'static str,
    pub(crate) local_lights: u32,
    pub(crate) shadowed: bool,
}

impl BenchmarkScenario {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        let (name, local_lights, shadowed) = match name {
            "single_spot" => ("single_spot", 1, true),
            "many_lights_16" => ("many_lights_16", 16, false),
            "many_lights_64" => ("many_lights_64", 64, false),
            "many_lights_256" => ("many_lights_256", 256, false),
            _ => return None,
        };
        Some(Self {
            name,
            local_lights,
            shadowed,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct VisualDiffReport {
    pub(crate) mae: f64,
    pub(crate) rmse: f64,
    pub(crate) max_error: u8,
    pub(crate) matches: bool,
}

pub(crate) fn compare_png(
    reference_path: &std::path::Path,
    candidate_path: &std::path::Path,
    diff_path: &std::path::Path,
    tolerance: f64,
) -> Result<VisualDiffReport, String> {
    let reference = image::open(reference_path)
        .map_err(|error| error.to_string())?
        .to_rgb8();
    let candidate = image::open(candidate_path)
        .map_err(|error| error.to_string())?
        .to_rgb8();
    if reference.dimensions() != candidate.dimensions() {
        return Err("reference and candidate dimensions differ".to_string());
    }
    let mut absolute_sum = 0_u64;
    let mut squared_sum = 0_u64;
    let mut max_error = 0_u8;
    let mut channels = 0_usize;
    let mut diff = image::RgbImage::new(reference.width(), reference.height());
    for (x, y, expected) in reference.enumerate_pixels() {
        let actual = candidate.get_pixel(x, y);
        for (expected_channel, actual_channel) in expected.0.iter().zip(actual.0) {
            let error = expected_channel.abs_diff(actual_channel);
            absolute_sum += u64::from(error);
            squared_sum += u64::from(error) * u64::from(error);
            max_error = max_error.max(error);
            channels += 1;
        }
        let error = expected
            .0
            .iter()
            .zip(actual.0)
            .map(|(left, right)| left.abs_diff(right))
            .max()
            .unwrap_or(0);
        diff.put_pixel(x, y, image::Rgb([error.saturating_mul(4), 0, 0]));
    }
    if let Some(parent) = diff_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    diff.save(diff_path).map_err(|error| error.to_string())?;
    let count = channels.max(1) as f64;
    let mae = absolute_sum as f64 / count;
    let rmse = (squared_sum as f64 / count).sqrt();
    Ok(VisualDiffReport {
        mae,
        rmse,
        max_error,
        matches: mae <= tolerance && max_error <= 2,
    })
}

pub(crate) fn pixel_matches(
    image_path: &std::path::Path,
    x: u32,
    y: u32,
    expected: [u8; 3],
    tolerance: u8,
) -> Result<bool, String> {
    let image = image::open(image_path)
        .map_err(|error| error.to_string())?
        .to_rgb8();
    if x >= image.width() || y >= image.height() {
        return Err("pixel probe is outside the image".to_string());
    }
    let actual = image.get_pixel(x, y).0;
    Ok(actual
        .iter()
        .zip(expected)
        .all(|(left, right)| left.abs_diff(right) <= tolerance))
}

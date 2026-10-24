//! Build-time WESL shader validation for the `moonly` workspace.

use std::error::Error;

const WESL_PACKAGES: &[(&str, &str, &str)] = &[
    (
        "crates/moonly_core/src/core_common.wesl",
        "crates/moonly_core/src",
        "package::core_common",
    ),
    (
        "crates/moonly_shadows/src/shadows.wesl",
        "crates/moonly_shadows/src",
        "package::shadows",
    ),
    (
        "crates/moonly_gi/src/gi.wesl",
        "crates/moonly_gi/src",
        "package::gi",
    ),
    (
        "crates/moonly_volumetrics/src/volumetrics.wesl",
        "crates/moonly_volumetrics/src",
        "package::volumetrics",
    ),
    (
        "crates/moonly_post/src/tonemap.wesl",
        "crates/moonly_post/src",
        "package::tonemap",
    ),
    (
        "crates/moonly_post/src/bloom_optics.wesl",
        "crates/moonly_post/src",
        "package::bloom_optics",
    ),
];

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=crates/moonly_post/src/moonly_lit.wesl");
    for &(file_path, base_dir, module_path) in WESL_PACKAGES {
        println!("cargo::rerun-if-changed={file_path}");
        let parsed = module_path.parse()?;
        wesl::Wesl::new(base_dir).compile(&parsed)?;
    }
    Ok(())
}

#![allow(
    warnings,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unwrap_used
)]

include!(concat!(
    env!("OUT_DIR"),
    "/compat_examples/bevy_tonemapping.rs"
));

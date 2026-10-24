//! Shader parameter bus (`shader_param_1..8`) ported from `docs/shader_bus.md`.

use bevy::{prelude::*, render::render_resource::ShaderType};

/// Number of `vec4<f32>` uniform slots in the XRay-Monolith shader bus.
pub const SHADER_BUS_SLOT_COUNT: usize = 8;

/// GPU-compatible shader bus uniform block (`shader_param_1` through `shader_param_8`).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Reflect, ShaderType)]
#[reflect(Resource)]
pub struct ShaderBus {
    /// Eight 4-component float registers exposed to WESL shaders.
    pub params: [Vec4; SHADER_BUS_SLOT_COUNT],
}

impl Default for ShaderBus {
    fn default() -> Self {
        Self {
            params: [Vec4::ZERO; SHADER_BUS_SLOT_COUNT],
        }
    }
}

impl ShaderBus {
    /// Read a 1-indexed shader bus slot (`1..=8`) matching `shader_param_N`.
    #[must_use]
    pub fn get_slot(&self, one_based_index: usize) -> Option<Vec4> {
        let idx = one_based_index.checked_sub(1)?;
        self.params.get(idx).copied()
    }

    /// Write a `Vec4` value into a 1-indexed shader bus slot (`1..=8`).
    pub fn set_slot(&mut self, one_based_index: usize, value: Vec4) -> bool {
        let Some(idx) = one_based_index.checked_sub(1) else {
            return false;
        };
        let Some(slot) = self.params.get_mut(idx) else {
            return false;
        };
        *slot = value;
        true
    }

    /// Write a linear RGBA color into a 1-indexed shader bus slot (`1..=8`).
    pub fn set_color(&mut self, one_based_index: usize, rgba: LinearRgba) -> bool {
        self.set_slot(
            one_based_index,
            Vec4::new(rgba.red, rgba.green, rgba.blue, rgba.alpha),
        )
    }
}

//! Custom Bevy `Material` bound to `moonly_lit.wesl` with automatic XRay-Monolith uniform sync.

use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use moonly_core::{
    AmbientOcclusionMode, LightPackage, MoonlyConfig, ShadowFilterMode, WeatherEnvironment,
};
use moonly_gi::GiLightField;
use moonly_volumetrics::{SunShaftsConfig, SunShaftsMode};

use crate::{Hdr10PostConfig, OpticsMode};

const LIT_SHADER_PATH: &str = "embedded://moonly_post/moonly_lit.wesl";

/// GPU uniform layout matching `MoonlyLitUniform` in `moonly_lit.wesl`.
#[derive(Debug, Clone, Copy, PartialEq, Reflect, ShaderType)]
pub struct MoonlyLitUniform {
    /// Linear RGB surface albedo (`xyz`) and roughness (`w`).
    pub albedo_roughness: Vec4,
    /// Emissive radiance (`xyz`) and scalar ambient occlusion (`w`).
    pub emissive_ao: Vec4,
    /// Sun direction (`xyz`) and shadow filter softness (`w`).
    pub sun_dir_shadow_softness: Vec4,
    /// Sun linear RGB color (`xyz`) and rain wetness (`w`).
    pub sun_color_wetness: Vec4,
    /// Hemisphere sky color (`xyz`) and hemisphere intensity (`w`).
    pub hemi_color: Vec4,
    /// Base ambient floor color (`xyz`) and distance fog density (`w`).
    pub ambient_fog_density: Vec4,
    /// Primary spot light position (`xyz`) and range (`w`).
    pub spot_pos_range: Vec4,
    /// Primary spot light direction (`xyz`) and outer cone cosine (`w`).
    pub spot_dir_cos: Vec4,
    /// Primary spot light color (`xyz`) and volumetric weight (`w`).
    pub spot_color_vol: Vec4,
    /// Virtual GI bounce light 0 position (`xyz`) and radius (`w`).
    pub gi_pos_range_0: Vec4,
    /// Virtual GI bounce light 0 color (`xyz`).
    pub gi_color_0: Vec4,
    /// Virtual GI bounce light 1 position (`xyz`) and radius (`w`).
    pub gi_pos_range_1: Vec4,
    /// Virtual GI bounce light 1 color (`xyz`).
    pub gi_color_1: Vec4,
    /// Virtual GI bounce light 2 position (`xyz`) and radius (`w`).
    pub gi_pos_range_2: Vec4,
    /// Virtual GI bounce light 2 color (`xyz`).
    pub gi_color_2: Vec4,
    /// Analytical 3D shadow blocker 0 (`w > 0`: sphere radius, `w < 0`: cylinder `-radius` with top `y`).
    pub blocker_0: Vec4,
    /// Analytical 3D shadow blocker 1.
    pub blocker_1: Vec4,
    /// Analytical 3D shadow blocker 2.
    pub blocker_2: Vec4,
    /// Analytical 3D shadow blocker 3.
    pub blocker_3: Vec4,
    /// Exposure (`x`), contrast (`y`), vibrance (`z`), and sunshaft density (`w`).
    pub post_params: Vec4,
    /// Shaft density (`x`), anisotropy (`y`), screen decay (`z`), and ray steps (`w`).
    pub shaft_params: Vec4,
    /// Tonemapper index (`x`), optics mode (`y`), shadow filter (`z`), and AO mode (`w`).
    pub mode_flags: UVec4,
}

impl Default for MoonlyLitUniform {
    fn default() -> Self {
        Self {
            albedo_roughness: Vec4::new(0.75, 0.72, 0.68, 0.42),
            emissive_ao: Vec4::new(0.0, 0.0, 0.0, 1.0),
            sun_dir_shadow_softness: Vec4::new(-0.45, -0.78, -0.43, 1.0),
            sun_color_wetness: Vec4::new(1.35, 1.18, 0.95, 0.0),
            hemi_color: Vec4::new(0.42, 0.50, 0.64, 0.85),
            ambient_fog_density: Vec4::new(0.06, 0.07, 0.09, 0.012),
            spot_pos_range: Vec4::ZERO,
            spot_dir_cos: Vec4::ZERO,
            spot_color_vol: Vec4::ZERO,
            gi_pos_range_0: Vec4::ZERO,
            gi_color_0: Vec4::ZERO,
            gi_pos_range_1: Vec4::ZERO,
            gi_color_1: Vec4::ZERO,
            gi_pos_range_2: Vec4::ZERO,
            gi_color_2: Vec4::ZERO,
            blocker_0: Vec4::new(0.0, 1.15, 0.0, 0.85),
            blocker_1: Vec4::new(-2.2, 3.6, 0.0, -0.75),
            blocker_2: Vec4::new(2.2, 3.6, 0.0, -0.75),
            blocker_3: Vec4::new(0.0, 0.5, 0.0, -1.10),
            post_params: Vec4::new(1.1, 0.05, 0.08, 0.35),
            shaft_params: Vec4::new(0.35, 0.68, 0.96, 24.0),
            mode_flags: UVec4::new(4, 0, 1, 3),
        }
    }
}

/// Bevy 3D material that shades meshes with `moonly_lit.wesl`.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
pub struct MoonlyLitMaterial {
    /// Packed uniform buffer bound at `@group(MATERIAL_BIND_GROUP) @binding(0)`.
    #[uniform(0)]
    pub uniform: MoonlyLitUniform,
}

impl MoonlyLitMaterial {
    /// Create a material with the given linear RGB `albedo` and `roughness`.
    #[must_use]
    pub fn from_albedo_roughness(albedo: Vec3, roughness: f32) -> Self {
        let mut mat = Self::default();
        mat.uniform.albedo_roughness = albedo.extend(roughness.clamp(0.04, 1.0));
        mat
    }

    /// Override the 4 analytical 3D shadow blockers (`w > 0`: sphere, `w < 0`: vertical pillar).
    #[must_use]
    pub const fn with_blockers(mut self, blockers: [Vec4; 4]) -> Self {
        self.uniform.blocker_0 = blockers[0];
        self.uniform.blocker_1 = blockers[1];
        self.uniform.blocker_2 = blockers[2];
        self.uniform.blocker_3 = blockers[3];
        self
    }
}

impl Material for MoonlyLitMaterial {
    fn vertex_shader() -> ShaderRef {
        LIT_SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        LIT_SHADER_PATH.into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn enable_oit() -> bool {
        false
    }
}

/// Resource bundle type alias to keep `sync_moonly_lit_materials` under Clippy complexity limits.
pub type CoreLightingRes<'w> = (
    Res<'w, MoonlyConfig>,
    Res<'w, WeatherEnvironment>,
    Res<'w, LightPackage>,
    Res<'w, GiLightField>,
);

/// Post-process resource bundle type alias for `sync_moonly_lit_materials`.
pub type PostLightingRes<'w> = (
    Res<'w, SunShaftsConfig>,
    Res<'w, Hdr10PostConfig>,
    Res<'w, OpticsMode>,
);

pub(crate) struct LightingUniformParams<'a> {
    pub config: &'a MoonlyConfig,
    pub env: &'a WeatherEnvironment,
    pub package: &'a LightPackage,
    pub gi_field: &'a GiLightField,
    pub shafts: &'a SunShaftsConfig,
    pub hdr10: &'a Hdr10PostConfig,
    pub optics: &'a OpticsMode,
}

/// Synchronize global XRay-Monolith lighting, GI, and HDR10 resources into all `MoonlyLitMaterial` assets.
pub fn sync_moonly_lit_materials(
    core_res: CoreLightingRes,
    post_res: PostLightingRes,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
) {
    let (config, env, package, gi_field) = core_res;
    let (shafts, hdr10, optics) = post_res;
    let params = LightingUniformParams {
        config: &config,
        env: &env,
        package: &package,
        gi_field: &gi_field,
        shafts: &shafts,
        hdr10: &hdr10,
        optics: &optics,
    };
    for (_, material) in materials.iter_mut() {
        let mut next = material.uniform;
        apply_lighting_uniform(&mut next, &params);
        if next != material.uniform {
            material.uniform = next;
        }
    }
}

pub(crate) fn apply_lighting_uniform(u: &mut MoonlyLitUniform, params: &LightingUniformParams<'_>) {
    let (softness, shadow_idx) = match params.config.shadow_filter {
        ShadowFilterMode::HardwarePcf => (0.35, 0),
        ShadowFilterMode::MonolithJitter64 => (1.0, 1),
        ShadowFilterMode::MonolithMinMax => (1.0, 2),
        ShadowFilterMode::EnhancedPcss => (1.45, 3),
    };
    let ao_idx = match params.config.ao_mode {
        AmbientOcclusionMode::Off => 0,
        AmbientOcclusionMode::Ssao => 1,
        AmbientOcclusionMode::Hdao => 2,
        AmbientOcclusionMode::ActivisionColoredAo | AmbientOcclusionMode::EnhancedSsdo => 3,
    };
    let shaft_density = if matches!(params.shafts.mode, SunShaftsMode::Off) {
        0.0
    } else {
        params.shafts.density
    };
    let (sun_direction, sun_color) = params
        .package
        .v_direct
        .first()
        .map_or((params.env.sun_direction, params.env.sun_color), |light| {
            (light.direction, light.color)
        });
    u.sun_dir_shadow_softness = sun_direction.extend(softness);
    u.sun_color_wetness = sun_color.extend(params.config.rain_wetness);
    u.hemi_color = params.env.hemi_color;
    u.ambient_fog_density = params.env.ambient_color.extend(0.012);
    u.post_params = Vec4::new(
        params.hdr10.exposure,
        params.hdr10.contrast,
        params.hdr10.vibrance,
        shaft_density,
    );
    u.shaft_params = Vec4::new(
        shaft_density,
        params.shafts.mie_anisotropy,
        params.shafts.screen_space_decay,
        params.shafts.raymarch_steps as f32,
    );
    u.mode_flags = UVec4::new(
        params.hdr10.operator.shader_index(),
        params.optics.shader_index(),
        shadow_idx,
        ao_idx,
    );
    apply_spot_and_gi_uniforms(u, params.package, params.gi_field);
}

fn apply_spot_and_gi_uniforms(
    u: &mut MoonlyLitUniform,
    package: &LightPackage,
    gi_field: &GiLightField,
) {
    u.spot_pos_range = Vec4::ZERO;
    u.spot_dir_cos = Vec4::ZERO;
    u.spot_color_vol = Vec4::ZERO;
    if let Some(spot) = package
        .v_shadowed
        .first()
        .or_else(|| package.v_spot.first())
    {
        u.spot_pos_range = spot.position.extend(spot.range);
        u.spot_dir_cos = spot.direction.extend((spot.cone_angle * 0.5).cos());
        u.spot_color_vol = spot.color.extend(spot.volumetric.intensity);
    }
    let gi_slots = [
        (&mut u.gi_pos_range_0, &mut u.gi_color_0),
        (&mut u.gi_pos_range_1, &mut u.gi_color_1),
        (&mut u.gi_pos_range_2, &mut u.gi_color_2),
    ];
    for (idx, (pos_range, color)) in gi_slots.into_iter().enumerate() {
        *pos_range = Vec4::ZERO;
        *color = Vec4::ZERO;
        if let Some(light) = gi_field.virtual_lights.get(idx) {
            *pos_range = light.position.extend(light.range);
            *color = light.color.extend(1.0);
        }
    }
}

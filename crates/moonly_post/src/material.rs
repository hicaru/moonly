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
use moonly_volumetrics::SunShaftsConfig;

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
    /// Exposure (`x`), contrast (`y`), vibrance (`z`), and sunshaft density (`w`).
    pub post_params: Vec4,
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
            spot_pos_range: Vec4::new(0.0, 4.5, 0.0, 16.0),
            spot_dir_cos: Vec4::new(0.0, -1.0, 0.0, 0.65),
            spot_color_vol: Vec4::new(2.2, 1.6, 0.9, 0.4),
            gi_pos_range_0: Vec4::new(0.0, 0.2, 0.0, 5.0),
            gi_color_0: Vec4::new(0.35, 0.22, 0.12, 1.0),
            gi_pos_range_1: Vec4::new(-2.0, 1.0, 0.0, 4.5),
            gi_color_1: Vec4::new(0.18, 0.28, 0.36, 1.0),
            gi_pos_range_2: Vec4::new(2.0, 1.0, 0.0, 4.5),
            gi_color_2: Vec4::new(0.28, 0.20, 0.16, 1.0),
            post_params: Vec4::new(1.1, 0.05, 0.08, 0.35),
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

/// Synchronize global XRay-Monolith lighting, GI, and HDR10 resources into all `MoonlyLitMaterial` assets.
pub fn sync_moonly_lit_materials(
    core_res: CoreLightingRes,
    post_res: PostLightingRes,
    mut materials: ResMut<Assets<MoonlyLitMaterial>>,
) {
    let (config, env, package, gi_field) = core_res;
    let (shafts, hdr10, optics) = post_res;

    let softness = match config.shadow_filter {
        ShadowFilterMode::HardwarePcf => 0.35,
        ShadowFilterMode::MonolithJitter64 | ShadowFilterMode::MonolithMinMax => 1.0,
        ShadowFilterMode::EnhancedPcss => 1.45,
    };
    let ao_idx = match config.ao_mode {
        AmbientOcclusionMode::Off => 0,
        AmbientOcclusionMode::Ssao => 1,
        AmbientOcclusionMode::Hdao => 2,
        AmbientOcclusionMode::ActivisionColoredAo | AmbientOcclusionMode::EnhancedSsdo => 3,
    };
    let flags = UVec4::new(
        hdr10.operator.shader_index(),
        optics.shader_index(),
        1,
        ao_idx,
    );

    let ids: Vec<AssetId<MoonlyLitMaterial>> = materials.ids().collect();
    for id in ids {
        let Some(existing) = materials.get(id) else {
            continue;
        };
        let mut next = existing.uniform;
        next.sun_dir_shadow_softness = env.sun_direction.extend(softness);
        next.sun_color_wetness = env.sun_color.extend(config.rain_wetness);
        next.hemi_color = env.hemi_color;
        next.ambient_fog_density = env.ambient_color.extend(0.012);
        next.post_params = Vec4::new(
            hdr10.exposure,
            hdr10.contrast,
            hdr10.vibrance,
            shafts.density,
        );
        next.mode_flags = flags;
        apply_spot_and_gi_uniforms(&mut next, &package, &gi_field);
        if next != existing.uniform
            && let Some(mut mat) = materials.get_mut(id)
        {
            mat.uniform = next;
        }
    }
}

fn apply_spot_and_gi_uniforms(
    u: &mut MoonlyLitUniform,
    package: &LightPackage,
    gi_field: &GiLightField,
) {
    if let Some(spot) = package
        .v_shadowed
        .first()
        .or_else(|| package.v_spot.first())
    {
        u.spot_pos_range = spot.position.extend(spot.range);
        u.spot_dir_cos = spot.direction.extend((spot.cone_angle * 0.5).cos());
        u.spot_color_vol = spot.color.extend(spot.volumetric.intensity);
    }
    if let Some(g0) = gi_field.virtual_lights.first() {
        u.gi_pos_range_0 = g0.position.extend(g0.range);
        u.gi_color_0 = (g0.color * 12.0).extend(1.0);
    }
    if let Some(g1) = gi_field.virtual_lights.get(1) {
        u.gi_pos_range_1 = g1.position.extend(g1.range);
        u.gi_color_1 = (g1.color * 12.0).extend(1.0);
    }
    if let Some(g2) = gi_field.virtual_lights.get(2) {
        u.gi_pos_range_2 = g2.position.extend(g2.range);
        u.gi_color_2 = (g2.color * 12.0).extend(1.0);
    }
}

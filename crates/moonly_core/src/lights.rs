//! Light components and batching packages ported from `light.h` and `Light_Package.h`.

use bevy::{
    prelude::*,
    render::{RenderApp, extract_resource::ExtractResource},
};

/// Light source classification matching `IRender_Light::LT` in `moonly-monolith`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum MoonlyLightType {
    /// Directional sun or moon light (`LT_DIRECT = 0`).
    Direct,
    /// Omnidirectional point light (`LT_POINT = 1`).
    #[default]
    Point,
    /// Conical spot or projective light (`LT_SECONDARY / Spot = 2`).
    Spot,
    /// Single tetrahedron or cube face of a split omnidirectional light (`Omnipart = 3`).
    Omnipart,
}

/// Bitflag constants matching `R_light` flags in `moonly-monolith`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub struct MoonlyLightFlags(pub u32);

impl MoonlyLightFlags {
    /// Light casts dynamic shadows (`bShadow`).
    pub const SHADOW: Self = Self(1 << 0);
    /// Light renders volumetric shafts (`bVolumetric`).
    pub const VOLUMETRIC: Self = Self(1 << 1);
    /// Light participates in player HUD illumination (`bHudMode`).
    pub const HUD_MODE: Self = Self(1 << 2);
    /// Light emits real-time GI radiosity bounce photons (`bGI`).
    pub const GI_ENABLED: Self = Self(1 << 3);
    /// Light is a virtual indirect bounce source (`bVirtualGI`).
    pub const VIRTUAL_GI: Self = Self(1 << 4);

    /// Check whether all bits in `other` are present.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Insert flag bits into this set.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Volumetric parameters for a spot or directional light (`light.h`).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct VolumetricLightSettings {
    /// Volumetric scattering intensity in `[0.0, 2.0]`.
    pub intensity: f32,
    /// Normalized raymarch distance multiplier in `[0.0, 1.0]`.
    pub distance: f32,
    /// Raymarch quality factor in `[0.1, 1.0]`.
    pub quality: f32,
}

impl Default for VolumetricLightSettings {
    fn default() -> Self {
        Self {
            intensity: 0.4,
            distance: 1.0,
            quality: 0.75,
        }
    }
}

/// Dynamic light component ported from `light` in `src/Layers/xrRender/light.h`.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct MoonlyLight {
    /// Light classification (`Direct`, `Point`, `Spot`, `Omnipart`).
    pub light_type: MoonlyLightType,
    /// Packed behavior flags (`SHADOW`, `VOLUMETRIC`, `GI_ENABLED`, etc.).
    pub flags: MoonlyLightFlags,
    /// Linear RGB light color multiplied by intensity.
    pub color: Vec3,
    /// World-space light position (`position` in `R_light`).
    pub position: Vec3,
    /// Normalized world-space forward direction (`direction` in `R_light`).
    pub direction: Vec3,
    /// Normalized world-space right vector for spot orientation (`right` in `R_light`).
    pub right: Vec3,
    /// Maximum attenuation radius in meters (`range` in `R_light`).
    pub range: f32,
    /// Outer spot cone angle in radians (`cone` in `R_light`).
    pub cone_angle: f32,
    /// Explicit shadow map resolution override (`0` uses automatic SSA sizing).
    pub smap_size_override: u16,
    /// Stable source entity for cache and render identity.
    pub source_entity: Option<Entity>,
    /// Original point light for an omnipart face.
    pub omnipart_parent: Option<Entity>,
    /// Cube face index in `0..6` for an omnipart light.
    pub omnipart_face: Option<u8>,
    /// Volumetric shaft parameters when `MoonlyLightFlags::VOLUMETRIC` is set.
    pub volumetric: VolumetricLightSettings,
}

impl Default for MoonlyLight {
    fn default() -> Self {
        Self {
            light_type: MoonlyLightType::Point,
            flags: MoonlyLightFlags::SHADOW.union(MoonlyLightFlags::GI_ENABLED),
            color: Vec3::splat(1.0),
            position: Vec3::new(0.0, 3.0, 0.0),
            direction: Vec3::NEG_Y,
            right: Vec3::X,
            range: 12.0,
            cone_angle: std::f32::consts::FRAC_PI_3,
            smap_size_override: 0,
            source_entity: None,
            omnipart_parent: None,
            omnipart_face: None,
            volumetric: VolumetricLightSettings::default(),
        }
    }
}

impl MoonlyLight {
    /// Create a shadow-casting spot light with GI radiosity enabled.
    #[must_use]
    pub fn spot(position: Vec3, direction: Vec3, color: Vec3, range: f32, cone_angle: f32) -> Self {
        let dir = direction.normalize_or_zero();
        let up = if dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
        let right = up.cross(dir).normalize_or_zero();
        Self {
            light_type: MoonlyLightType::Spot,
            flags: MoonlyLightFlags::SHADOW
                .union(MoonlyLightFlags::GI_ENABLED)
                .union(MoonlyLightFlags::VOLUMETRIC),
            color,
            position,
            direction: dir,
            right,
            range,
            cone_angle,
            ..Self::default()
        }
    }

    /// Compute the specular weight `u_diffuse2s(color)` from `common_functions.h`.
    #[must_use]
    pub fn specular_weight(&self) -> f32 {
        let v = self.color;
        (v.x * v.x + v.y * v.y + v.z * v.z).sqrt() / 3.0_f32.sqrt()
    }
}

/// Virtual indirect bounce light generated by `light::gi_generate` (`light_GI.h`).
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct VirtualGiLight {
    /// World-space hit position. Enhanced mode applies a normal standoff.
    pub position: Vec3,
    /// Reflected ray direction in Monolith mode. Surface normal in enhanced mode.
    pub direction: Vec3,
    /// Normalized energy in Monolith mode. Albedo-weighted energy in enhanced mode.
    pub color: Vec3,
    /// Virtual light radius derived from energy and pipeline mode.
    pub range: f32,
}

/// Complete virtual light set prepared for reflected-light accumulation.
#[derive(Resource, Debug, Default, Clone, Reflect)]
#[reflect(Resource)]
pub struct VirtualGiLightPackage {
    pub lights: Vec<VirtualGiLight>,
}

impl ExtractResource<RenderApp> for VirtualGiLightPackage {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Batched light buckets for deferred accumulation (`Light_Package` in `Light_Package.h`).
#[derive(Resource, Debug, Default, Clone, Reflect)]
#[reflect(Resource)]
pub struct LightPackage {
    /// Unshadowed omnidirectional point lights (`v_point`).
    pub v_point: Vec<MoonlyLight>,
    /// Unshadowed conical spot lights (`v_spot`).
    pub v_spot: Vec<MoonlyLight>,
    /// Shadow-casting spot or omnipart lights (`v_shadowed`).
    pub v_shadowed: Vec<MoonlyLight>,
    /// Directional lights handled by the sun cascade pass.
    pub v_direct: Vec<MoonlyLight>,
}

impl ExtractResource<RenderApp> for LightPackage {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

impl LightPackage {
    /// Clear all light lists while keeping allocated capacity.
    pub fn clear(&mut self) {
        self.v_point.clear();
        self.v_spot.clear();
        self.v_shadowed.clear();
        self.v_direct.clear();
    }

    /// Sort a single light into its explicit accumulation or sun bucket.
    pub fn push(&mut self, mut light: MoonlyLight) {
        if matches!(light.light_type, MoonlyLightType::Direct) {
            self.v_direct.push(light);
            return;
        }
        if light.flags.contains(MoonlyLightFlags::SHADOW) {
            if matches!(light.light_type, MoonlyLightType::Point) {
                // Split point light into 6 cube-face spot frustums like Omnipart in R4.
                for (face, (dir, right)) in omnipart_axes().into_iter().enumerate() {
                    let mut part = light.clone();
                    part.light_type = MoonlyLightType::Omnipart;
                    part.direction = dir;
                    part.right = right;
                    part.cone_angle = std::f32::consts::FRAC_PI_2;
                    part.omnipart_parent = light.source_entity;
                    part.omnipart_face = Some(face as u8);
                    self.v_shadowed.push(part);
                }
            } else {
                self.v_shadowed.push(light);
            }
            return;
        }
        light.flags = MoonlyLightFlags(light.flags.0 & !MoonlyLightFlags::SHADOW.0);
        match light.light_type {
            MoonlyLightType::Point => self.v_point.push(light),
            MoonlyLightType::Spot | MoonlyLightType::Omnipart => self.v_spot.push(light),
            MoonlyLightType::Direct => self.v_direct.push(light),
        }
    }

    /// Return the total count of active lights across all buckets.
    #[must_use]
    pub const fn total_count(&self) -> usize {
        self.v_point.len() + self.v_spot.len() + self.v_shadowed.len() + self.v_direct.len()
    }
}

const fn omnipart_axes() -> [(Vec3, Vec3); 6] {
    [
        (Vec3::X, Vec3::Z),
        (Vec3::NEG_X, Vec3::NEG_Z),
        (Vec3::Y, Vec3::X),
        (Vec3::NEG_Y, Vec3::NEG_X),
        (Vec3::Z, Vec3::NEG_X),
        (Vec3::NEG_Z, Vec3::X),
    ]
}

/// Synchronize `GlobalTransform` into `MoonlyLight` and populate the frame `LightPackage`.
pub fn collect_light_packages(
    mut package: ResMut<LightPackage>,
    mut query: Query<(Entity, &mut MoonlyLight, Option<&GlobalTransform>)>,
) {
    package.clear();
    for (entity, mut light, transform) in &mut query {
        light.source_entity = Some(entity);
        if let Some(xf) = transform {
            light.position = xf.translation();
            light.direction = xf.forward().as_vec3();
            light.right = xf.right().as_vec3();
        }
        package.push(light.clone());
    }
}

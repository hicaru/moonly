use bevy::prelude::*;

use crate::gi_bounce::GiReflectorBox;
use crate::gi_intersection::{raycast_all_reflectors, raycast_any_reflector};

/// World-space ray bounds for a GI nearest-hit query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GiRayQuery {
    /// Ray origin in world space.
    pub origin: Vec3,
    /// Normalized ray direction.
    pub direction: Vec3,
    /// Minimum accepted hit distance in meters.
    pub min_distance: f32,
    /// Maximum accepted hit distance in meters.
    pub max_distance: f32,
}

impl GiRayQuery {
    /// Create a ray query with explicit distance bounds.
    #[must_use]
    pub const fn new(origin: Vec3, direction: Vec3, min_distance: f32, max_distance: f32) -> Self {
        Self {
            origin,
            direction,
            min_distance,
            max_distance,
        }
    }
}

/// Geometric result of a GI nearest-hit query.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GiRayHit {
    /// Distance from the ray origin in meters.
    pub distance: f32,
    /// Normalized geometric surface normal.
    pub normal: Vec3,
    /// Surface albedo for the enhanced model.
    pub albedo: Vec3,
}

/// Nearest-hit geometry shared by AABB and triangle GI generation.
pub trait GiRayGeometry {
    /// Return the nearest valid hit within the ray bounds.
    fn nearest_hit(&self, ray: GiRayQuery) -> Option<GiRayHit>;

    /// Return `true` when any valid hit exists within the ray bounds.
    fn any_hit(&self, ray: GiRayQuery) -> bool {
        self.nearest_hit(ray).is_some()
    }
}

impl GiRayGeometry for [GiReflectorBox] {
    fn nearest_hit(&self, ray: GiRayQuery) -> Option<GiRayHit> {
        raycast_all_reflectors(
            ray.origin,
            ray.direction,
            ray.max_distance,
            ray.min_distance,
            self,
        )
        .map(|(distance, normal, albedo)| GiRayHit {
            distance,
            normal,
            albedo,
        })
    }

    fn any_hit(&self, ray: GiRayQuery) -> bool {
        raycast_any_reflector(
            ray.origin,
            ray.direction,
            ray.max_distance,
            ray.min_distance,
            self,
        )
    }
}

impl<const N: usize> GiRayGeometry for [GiReflectorBox; N] {
    fn nearest_hit(&self, ray: GiRayQuery) -> Option<GiRayHit> {
        self.as_slice().nearest_hit(ray)
    }

    fn any_hit(&self, ray: GiRayQuery) -> bool {
        self.as_slice().any_hit(ray)
    }
}

pub(crate) struct GiCombinedGeometry<'a, G: GiRayGeometry + ?Sized> {
    pub(crate) triangles: &'a G,
    pub(crate) reflectors: &'a [GiReflectorBox],
}

impl<G: GiRayGeometry + ?Sized> GiRayGeometry for GiCombinedGeometry<'_, G> {
    fn nearest_hit(&self, ray: GiRayQuery) -> Option<GiRayHit> {
        let triangle = self.triangles.nearest_hit(ray);
        let narrowed = match triangle {
            Some(hit) => GiRayQuery {
                max_distance: hit.distance,
                ..ray
            },
            None => ray,
        };
        match self.reflectors.nearest_hit(narrowed) {
            Some(reflector) => Some(reflector),
            None => triangle,
        }
    }

    fn any_hit(&self, ray: GiRayQuery) -> bool {
        self.triangles.any_hit(ray) || self.reflectors.any_hit(ray)
    }
}

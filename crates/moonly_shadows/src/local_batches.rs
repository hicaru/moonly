use bevy::prelude::*;
use moonly_core::{LightPackage, MoonlyConfig, MoonlyPipelineTier};

use crate::{SmapAllocator, SmapRect, SpotShadowXform};

/// One shadow view allocation in a local-light atlas batch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalShadowView {
    /// Index in `LightPackage::v_shadowed`.
    pub light_index: usize,
    /// Light view and projection data.
    pub xform: SpotShadowXform,
    /// Allocated atlas viewport.
    pub rect: SmapRect,
}

/// One complete atlas batch.
#[derive(Debug, Default)]
pub struct LocalShadowBatch {
    pub allocator: SmapAllocator,
    pub views: Vec<LocalShadowView>,
}

impl LocalShadowBatch {
    fn clear(&mut self, pool_size: u32) {
        self.allocator.initialize(pool_size);
        self.views.clear();
    }

    #[must_use]
    pub fn utilization(&self) -> f32 {
        let capacity = self.allocator.pool_size.saturating_pow(2);
        if capacity == 0 {
            return 0.0;
        }
        let used = self
            .views
            .iter()
            .map(|view| view.rect.width().saturating_mul(view.rect.height()))
            .fold(0_u32, u32::saturating_add);
        used as f32 / capacity as f32
    }
}

/// Persistent local-light batches with reusable CPU storage.
#[derive(Resource, Debug, Default)]
pub struct LocalShadowBatches {
    pub batches: Vec<LocalShadowBatch>,
    active_batches: usize,
    previous_sizes: Vec<u32>,
}

impl LocalShadowBatches {
    /// Rebuild all batches and retain every valid shadow view.
    pub fn rebuild(&mut self, package: &LightPackage, config: &MoonlyConfig, camera_pos: Vec3) {
        let pool_size = config.smap_pool_size.max(8);
        self.active_batches = 0;
        for batch in &mut self.batches {
            batch.clear(pool_size);
        }
        self.previous_sizes.reserve(package.v_shadowed.len());
        for (light_index, light) in package.v_shadowed.iter().enumerate() {
            let previous = self.previous_sizes.get(light_index).copied().unwrap_or(0);
            let mut xform =
                SpotShadowXform::compute_xf_spot(light, camera_pos, pool_size, previous);
            xform.smap_size = xform.smap_size.clamp(5, pool_size);
            self.push_view(light_index, xform, config.tier, pool_size);
            self.record_size(light_index, xform.smap_size);
        }
        self.previous_sizes.truncate(package.v_shadowed.len());
    }

    fn push_view(
        &mut self,
        light_index: usize,
        xform: SpotShadowXform,
        tier: MoonlyPipelineTier,
        pool_size: u32,
    ) {
        loop {
            if self.active_batches >= self.batches.len() {
                self.batches.push(LocalShadowBatch::default());
                if let Some(batch) = self.batches.last_mut() {
                    batch.clear(pool_size);
                }
            }
            let Some(batch) = self.batches.get_mut(self.active_batches) else {
                return;
            };
            let allocated = if matches!(tier, MoonlyPipelineTier::MoonlyEnhanced) {
                batch.allocator.push_enhanced_bssf(xform.smap_size)
            } else {
                batch.allocator.push_monolith(xform.smap_size)
            };
            if let Some(rect) = allocated {
                batch.views.push(LocalShadowView {
                    light_index,
                    xform,
                    rect,
                });
                return;
            }
            self.active_batches += 1;
        }
    }

    fn record_size(&mut self, index: usize, size: u32) {
        if let Some(previous) = self.previous_sizes.get_mut(index) {
            *previous = size;
        } else {
            self.previous_sizes.push(size);
        }
    }

    #[must_use]
    pub fn batch_count(&self) -> usize {
        self.active_batches + usize::from(self.view_count() > 0)
    }

    #[must_use]
    pub fn view_count(&self) -> usize {
        self.batches.iter().map(|batch| batch.views.len()).sum()
    }

    #[must_use]
    pub fn active(&self) -> &[LocalShadowBatch] {
        let count = self.batch_count().min(self.batches.len());
        self.batches.get(..count).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use moonly_core::MoonlyLight;

    use super::*;

    #[test]
    fn overflow_creates_batches_without_losing_views() {
        let mut package = LightPackage::default();
        for _ in 0..5 {
            package.push(MoonlyLight {
                smap_size_override: 64,
                ..MoonlyLight::default()
            });
        }
        let config = MoonlyConfig {
            smap_pool_size: 128,
            ..MoonlyConfig::moonly_r4()
        };
        let mut batches = LocalShadowBatches::default();
        batches.rebuild(&package, &config, Vec3::ZERO);
        assert_eq!(batches.view_count(), 30);
        assert!(batches.batches.len() > 1);
        assert!(
            batches
                .batches
                .iter()
                .all(|batch| batch.utilization() <= 1.0)
        );
    }

    #[test]
    fn explicit_oversize_view_is_clamped_and_retained() {
        let mut package = LightPackage::default();
        package.push(MoonlyLight {
            light_type: moonly_core::MoonlyLightType::Spot,
            smap_size_override: 1024,
            ..MoonlyLight::default()
        });
        let config = MoonlyConfig {
            smap_pool_size: 128,
            ..MoonlyConfig::moonly_r4()
        };
        let mut batches = LocalShadowBatches::default();
        batches.rebuild(&package, &config, Vec3::ZERO);
        assert_eq!(batches.view_count(), 1);
        assert_eq!(
            batches.batches.first().map(|batch| batch.views.len()),
            Some(1)
        );
    }

    #[test]
    fn warm_rebuild_reuses_batch_and_view_storage() {
        let mut package = LightPackage::default();
        package.push(MoonlyLight {
            light_type: moonly_core::MoonlyLightType::Spot,
            smap_size_override: 64,
            ..MoonlyLight::default()
        });
        let config = MoonlyConfig {
            smap_pool_size: 128,
            ..MoonlyConfig::moonly_r4()
        };
        let mut batches = LocalShadowBatches::default();
        batches.rebuild(&package, &config, Vec3::ZERO);
        let batch_capacity = batches.batches.capacity();
        let view_capacities = batches
            .batches
            .iter()
            .map(|batch| batch.views.capacity())
            .collect::<Vec<_>>();
        batches.rebuild(&package, &config, Vec3::ZERO);
        assert_eq!(batches.batches.capacity(), batch_capacity);
        assert_eq!(batches.batch_count(), 1);
        assert!(
            batches
                .batches
                .iter()
                .zip(view_capacities)
                .all(|(batch, capacity)| batch.views.capacity() == capacity)
        );
    }
}

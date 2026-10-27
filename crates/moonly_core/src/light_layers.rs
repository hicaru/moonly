use bevy::camera::visibility::RenderLayers;

use crate::{MoonlyLight, VirtualGiLight};

impl VirtualGiLight {
    /// Check whether this virtual light shares a layer with the supplied view.
    #[must_use]
    pub fn visible_in_layers(&self, view: &RenderLayers) -> bool {
        view.bits().iter().enumerate().any(|(index, bits)| {
            self.render_layers
                .get(index)
                .is_some_and(|light_bits| light_bits & bits != 0)
        })
    }
}

impl MoonlyLight {
    /// Check whether this light shares a layer with the supplied view.
    #[must_use]
    pub fn visible_in_layers(&self, view: &RenderLayers) -> bool {
        view.bits().iter().enumerate().any(|(index, bits)| {
            self.render_layers
                .get(index)
                .is_some_and(|light_bits| light_bits & bits != 0)
        })
    }
}

pub(crate) fn sync_render_layers(light: &mut MoonlyLight, layers: Option<&RenderLayers>) {
    light.render_layers = [0; 4];
    if let Some(layers) = layers {
        for (index, bits) in layers.bits().iter().enumerate() {
            if let Some(target) = light.render_layers.get_mut(index) {
                *target = *bits;
            }
        }
        return;
    }
    if let Some(default) = light.render_layers.first_mut() {
        *default = 1;
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::Vec3;

    use super::*;

    #[test]
    fn layer_masks_match_only_intersecting_views() {
        let mut light = MoonlyLight::default();
        sync_render_layers(&mut light, Some(&RenderLayers::layer(1).with(63)));
        assert!(light.visible_in_layers(&RenderLayers::layer(1)));
        assert!(light.visible_in_layers(&RenderLayers::layer(63)));
        assert!(!light.visible_in_layers(&RenderLayers::layer(0)));
        assert!(!light.visible_in_layers(&RenderLayers::none()));
    }

    #[test]
    fn virtual_lights_inherit_source_layer_visibility() {
        let light = MoonlyLight::default();
        let virtual_light = VirtualGiLight {
            position: Vec3::ZERO,
            direction: Vec3::Y,
            color: Vec3::ONE,
            range: 1.0,
            render_layers: light.render_layers,
        };
        assert!(virtual_light.visible_in_layers(&RenderLayers::layer(0)));
        assert!(!virtual_light.visible_in_layers(&RenderLayers::layer(1)));
    }

    #[test]
    fn missing_layers_select_the_default_layer() {
        let mut light = MoonlyLight::default();
        sync_render_layers(&mut light, Some(&RenderLayers::layer(7)));
        sync_render_layers(&mut light, None);
        assert!(light.visible_in_layers(&RenderLayers::default()));
        assert!(!light.visible_in_layers(&RenderLayers::layer(7)));
    }
}

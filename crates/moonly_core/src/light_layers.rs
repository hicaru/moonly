use bevy::camera::visibility::RenderLayers;

use crate::MoonlyLight;

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
    fn missing_layers_select_the_default_layer() {
        let mut light = MoonlyLight::default();
        sync_render_layers(&mut light, Some(&RenderLayers::layer(7)));
        sync_render_layers(&mut light, None);
        assert!(light.visible_in_layers(&RenderLayers::default()));
        assert!(!light.visible_in_layers(&RenderLayers::layer(7)));
    }
}

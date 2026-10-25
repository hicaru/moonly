use bevy::{camera::visibility::RenderLayers, prelude::*};
use moonly_core::{LightPackage, MoonlyLight};

use crate::camera::MoonlyCamera;

#[derive(Component, Default)]
pub(crate) struct MoonlyViewLightPackage(pub LightPackage);

type ViewLightQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static RenderLayers>,
        Option<&'static mut MoonlyViewLightPackage>,
    ),
    With<MoonlyCamera>,
>;

pub(crate) fn prepare_view_light_packages(
    mut commands: Commands,
    source: Res<LightPackage>,
    mut views: ViewLightQuery,
) {
    let default_layers = RenderLayers::default();
    for (entity, layers, current) in &mut views {
        let layers = layers.unwrap_or(&default_layers);
        if let Some(mut current) = current {
            fill_view_package(&source, layers, &mut current.0);
            continue;
        }
        let mut package = LightPackage::default();
        fill_view_package(&source, layers, &mut package);
        commands
            .entity(entity)
            .insert(MoonlyViewLightPackage(package));
    }
}

fn fill_view_package(source: &LightPackage, layers: &RenderLayers, target: &mut LightPackage) {
    target.clear();
    extend_filtered(&mut target.v_point, &source.v_point, layers);
    extend_filtered(&mut target.v_spot, &source.v_spot, layers);
    extend_filtered(&mut target.v_shadowed, &source.v_shadowed, layers);
    extend_filtered(&mut target.v_direct, &source.v_direct, layers);
}

fn extend_filtered(target: &mut Vec<MoonlyLight>, source: &[MoonlyLight], layers: &RenderLayers) {
    target.extend(
        source
            .iter()
            .filter(|light| light.visible_in_layers(layers))
            .cloned(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_packages_include_only_intersecting_lights() {
        let mut source = LightPackage::default();
        source.v_point.push(MoonlyLight::default());
        source.v_point.push(MoonlyLight {
            render_layers: [2, 0, 0, 0],
            ..MoonlyLight::default()
        });
        let mut target = LightPackage::default();
        fill_view_package(&source, &RenderLayers::layer(1), &mut target);
        assert_eq!(target.v_point.len(), 1);
        assert_eq!(
            target.v_point.first().map(|light| light.render_layers),
            Some([2, 0, 0, 0])
        );
    }
}

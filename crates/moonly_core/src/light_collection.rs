use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::{LightPackage, MoonlyLight};

type MoonlyLightQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut MoonlyLight,
        Option<&'static GlobalTransform>,
        Option<&'static InheritedVisibility>,
        Option<&'static RenderLayers>,
    ),
>;

/// Synchronize visible `MoonlyLight` entities and populate the frame `LightPackage`.
pub fn collect_light_packages(
    mut package: ResMut<LightPackage>,
    native: Res<crate::MoonlyNativeForward>,
    mut query: MoonlyLightQuery,
) {
    if native.0 {
        package.clear();
        return;
    }
    package.clear();
    for (entity, mut light, transform, visibility, layers) in &mut query {
        if visibility.is_some_and(|visibility| !visibility.get()) {
            continue;
        }
        light.source_entity = Some(entity);
        crate::light_layers::sync_render_layers(&mut light, layers);
        if let Some(xf) = transform {
            light.position = xf.translation();
            light.direction = xf.forward().as_vec3();
            light.right = xf.right().as_vec3();
        }
        package.push(light.clone());
    }
}

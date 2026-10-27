use bevy::prelude::*;

use crate::forward_lighting::{
    ForwardBindings, ForwardPrepareInputs, ForwardViewQuery, MoonlyForwardView, fill_forward_view,
};

pub(crate) fn prepare_forward_bind_groups(
    mut commands: Commands,
    inputs: ForwardPrepareInputs,
    mut views: ForwardViewQuery,
) {
    for (entity, package, shadows, camera, current) in &mut views {
        let bindings = ForwardBindings {
            device: &inputs.device,
            queue: &inputs.queue,
            layout: &inputs.layout,
            cache: &inputs.cache,
            environment: &inputs.environment,
            camera,
            package: &package.0,
            shadows,
            cascades: &inputs.cascades,
        };
        let depth = shadows.first_depth();
        if let Some(mut current) = current {
            fill_forward_view(&mut current, &bindings, depth);
            continue;
        }
        let mut current = MoonlyForwardView::default();
        fill_forward_view(&mut current, &bindings, depth);
        commands.entity(entity).insert(current);
    }
}

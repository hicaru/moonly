use bevy::prelude::*;

use crate::forward_lighting::{
    ForwardPrepareInputs, ForwardViewQuery, MoonlyForwardView, fill_forward_view,
};

pub(crate) fn prepare_forward_bind_groups(
    mut commands: Commands,
    inputs: ForwardPrepareInputs,
    mut views: ForwardViewQuery,
) {
    for (entity, package, shadows, current) in &mut views {
        let depth = shadows.first_depth();
        if let Some(mut current) = current {
            fill_forward_view(
                &mut current,
                &inputs.device,
                &inputs.queue,
                &inputs.layout,
                &inputs.cache,
                &inputs.environment,
                &package.0,
                shadows,
                depth,
            );
            continue;
        }
        let mut current = MoonlyForwardView::default();
        fill_forward_view(
            &mut current,
            &inputs.device,
            &inputs.queue,
            &inputs.layout,
            &inputs.cache,
            &inputs.environment,
            &package.0,
            shadows,
            depth,
        );
        commands.entity(entity).insert(current);
    }
}

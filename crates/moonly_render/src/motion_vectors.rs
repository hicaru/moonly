use bevy::pbr::{MeshPipelineKey, RenderMeshInstanceFlags};

pub(crate) fn pipeline_key(enabled: bool, flags: RenderMeshInstanceFlags) -> MeshPipelineKey {
    if !enabled {
        return MeshPipelineKey::empty();
    }
    let mut key = MeshPipelineKey::MOTION_VECTOR_PREPASS;
    if flags.contains(RenderMeshInstanceFlags::HAS_PREVIOUS_SKIN) {
        key |= MeshPipelineKey::HAS_PREVIOUS_SKIN;
    }
    if flags.contains(RenderMeshInstanceFlags::HAS_PREVIOUS_MORPH) {
        key |= MeshPipelineKey::HAS_PREVIOUS_MORPH;
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_views_add_no_flags() {
        let flags = RenderMeshInstanceFlags::HAS_PREVIOUS_SKIN;
        assert_eq!(pipeline_key(false, flags), MeshPipelineKey::empty());
    }

    #[test]
    fn animated_meshes_preserve_previous_transform_data() {
        let flags = RenderMeshInstanceFlags::HAS_PREVIOUS_SKIN
            | RenderMeshInstanceFlags::HAS_PREVIOUS_MORPH;
        let key = pipeline_key(true, flags);
        assert!(key.contains(MeshPipelineKey::MOTION_VECTOR_PREPASS));
        assert!(key.contains(MeshPipelineKey::HAS_PREVIOUS_SKIN));
        assert!(key.contains(MeshPipelineKey::HAS_PREVIOUS_MORPH));
    }
}

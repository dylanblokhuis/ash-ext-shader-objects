use bevy::prelude::*;

use super::mesh::Mesh;

#[derive(Bundle, Clone)]
pub struct MaterialMeshBundle {
    pub mesh: Handle<Mesh>,
    // pub material: Handle<M>,
    pub transform: Transform,
    // pub global_transform: GlobalTransform,
    // /// User indication of whether an entity is visible
    // pub visibility: Visibility,
    // /// Algorithmically-computed indication of whether an entity is visible and should be extracted for rendering
    // pub computed_visibility: ComputedVisibility,
}

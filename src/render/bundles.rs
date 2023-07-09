use bevy::prelude::*;

use super::{material::Material, mesh::Mesh};

#[derive(Bundle, Clone, Debug)]
pub struct MaterialMeshBundle {
    pub mesh: Handle<Mesh>,
    pub material: Handle<Material>,
    pub transform: Transform,
    // pub global_transform: GlobalTransform,
    // /// User indication of whether an entity is visible
    // pub visibility: Visibility,
    // /// Algorithmically-computed indication of whether an entity is visible and should be extracted for rendering
    // pub computed_visibility: ComputedVisibility,
}

#[derive(Component, Clone, Default)]
pub struct Camera {
    pub projection: Mat4,
}

#[derive(Bundle, Clone, Default)]
pub struct CameraBundle {
    pub camera: Camera,
    pub transform: Transform,
}

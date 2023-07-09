use ash::vk;
use bevy::reflect::{TypePath, TypeUuid};

#[derive(Debug, TypeUuid, Clone, TypePath)]
#[uuid = "8ecbac0f-f545-4473-ad43-e1f4243af51e"]
pub struct Mesh {
    pub primitive_topology: vk::PrimitiveTopology,
    /// `std::collections::BTreeMap` with all defined vertex attributes (Positions, Normals, ...)
    /// for this mesh. Attribute ids to attribute values.
    /// Uses a BTreeMap because, unlike HashMap, it has a defined iteration order,
    /// which allows easy stable VertexBuffers (i.e. same buffer order)
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub tangent: [f32; 3],
    pub color: [f32; 4],
}

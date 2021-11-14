use anyhow::Result;

use crate::cmdl::Cmdl;
use crate::pak::Pak;

pub struct CanonicalMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
}

impl CanonicalMesh {
    pub fn new(_pak: &Pak, cmdl: &Cmdl, material_set_index: usize) -> Result<Self> {
        let mut positions = Vec::new();
        let mut normals = Vec::new();

        let material_set = &cmdl.materials[material_set_index];
        // TODO: Extract and use textures.

        for surface in &cmdl.surfaces {
            let material = &material_set.materials[surface.material_index as usize];
            let batches = surface.display_list.parse(
                material.vertex_attr_flags,
                &cmdl.position_data,
                &cmdl.normal_data,
            )?;
            for batch in batches {
                positions.extend_from_slice(&batch.positions);
                normals.extend_from_slice(&batch.normals);
            }
        }

        Ok(Self { positions, normals })
    }
}

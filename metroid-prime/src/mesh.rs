use anyhow::Result;

use crate::cmdl::Cmdl;
use crate::pak::Pak;

pub struct CanonicalMesh {
    pub surfaces: Vec<CanonicalMeshSurface>,
    pub texture_ids: Vec<u32>,
}

pub struct CanonicalMeshSurface {
    pub texture_indices: Vec<usize>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
}

impl CanonicalMesh {
    pub fn new(pak: &Pak, cmdl: &Cmdl, material_set_index: usize) -> Result<Self> {
        let material_set = &cmdl.materials[material_set_index];
        let mut surfaces = Vec::new();
        for surface in &cmdl.surfaces {
            let mut positions = Vec::new();
            let mut normals = Vec::new();
            let mut texcoords = Vec::new();

            let material = &material_set.materials[surface.material_index as usize];
            let batches = surface.display_list.parse(
                material.vertex_attr_flags,
                &cmdl.position_data,
                &cmdl.normal_data,
                &cmdl.uv_float_data,
            )?;
            for batch in batches {
                positions.extend_from_slice(&batch.positions);
                normals.extend_from_slice(&batch.normals);
                texcoords.extend_from_slice(&batch.texcoords);
            }

            surfaces.push(CanonicalMeshSurface {
                texture_indices: material
                    .texture_indices
                    .iter()
                    .map(|&x| x as usize)
                    .collect(),
                positions,
                normals,
                texcoords,
            });
        }

        Ok(Self {
            surfaces,
            texture_ids: material_set.texture_ids.clone(),
        })
    }
}

use anyhow::{anyhow, Result};
use gamecube::ReadTypedExt;

use crate::ancs::Ancs;
use crate::cinf::Cinf;
use crate::cmdl::Cmdl;
use crate::cskr::Cskr;
use crate::gx::{SkinnedVertexDescriptor, StaticVertexDescriptor};
use crate::pak::PakCache;

pub struct CanonicalMesh {
    pub skin: Option<CanonicalMeshSkin>,
    pub surfaces: Vec<CanonicalMeshSurface>,
    pub texture_ids: Vec<u32>,
}

pub struct CanonicalMeshSkin {
    pub skeleton: CanonicalMeshBone,
    pub skin: Cskr,
}

#[derive(Debug)]
pub struct CanonicalMeshBone {
    pub name: String,
    pub id: u32,
    pub position: [f32; 3],
    pub children: Vec<CanonicalMeshBone>,
}

pub struct CanonicalMeshSurface {
    pub texture_indices: Vec<usize>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub bone_ids: Vec<u32>,
    pub weights: Vec<f32>,
}

impl CanonicalMesh {
    pub fn from_cmdl(cmdl: &Cmdl, material_set_index: usize) -> Result<Self> {
        let material_set = &cmdl.materials[material_set_index];
        let mut surfaces = Vec::new();
        for surface in &cmdl.surfaces {
            let mut positions = Vec::new();
            let mut normals = Vec::new();
            let mut texcoords = Vec::new();

            let material = &material_set.materials[surface.material_index as usize];
            let batches = surface.display_list.parse::<StaticVertexDescriptor>(
                material.vertex_attr_flags,
                &cmdl.position_data,
                &cmdl.normal_data,
                &cmdl.uv_float_data,
                &(),
                &(),
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
                bone_ids: Vec::new(),
                weights: Vec::new(),
            });
        }

        Ok(Self {
            skin: None,
            surfaces,
            texture_ids: material_set.texture_ids.clone(),
        })
    }

    pub fn from_ancs(
        pak: &mut PakCache,
        ancs: &Ancs,
        character_index: usize,
        material_set_index: usize,
    ) -> Result<Self> {
        let character = &ancs.character_set.characters[character_index];

        let cmdl_data = pak
            .data_with_fourcc(character.model_id, "CMDL")?
            .ok_or_else(|| anyhow!("Model 0x{:08x} not found", character.model_id))?;
        let cmdl: Cmdl = cmdl_data.as_slice().read_typed()?;

        let skeleton_data = pak
            .data_with_fourcc(character.skeleton_id, "CINF")?
            .ok_or_else(|| anyhow!("Skeleton 0x{:08x} not found", character.skeleton_id))?;
        let skeleton: Cinf = skeleton_data.as_slice().read_typed()?;
        let skeleton = interpret_bone(&skeleton, skeleton.build_order_ids[0]);

        let skin_data = pak
            .data_with_fourcc(character.skin_id, "CSKR")?
            .ok_or_else(|| anyhow!("Skin 0x{:08x} not found", character.skin_id))?;
        let skin: Cskr = skin_data.as_slice().read_typed()?;
        let mut vertex_bone_ids = Vec::new();
        let mut vertex_weights = Vec::new();
        for vertex_group in &skin.vertex_groups {
            for _ in 0..vertex_group.vertex_count {
                assert_eq!(vertex_group.weights.len(), 1);
                vertex_bone_ids.push(vertex_group.weights[0].bone_id);
                vertex_weights.push(vertex_group.weights[0].weight);
            }
        }

        let material_set = &cmdl.materials[material_set_index];
        let mut surfaces = Vec::new();
        for surface in &cmdl.surfaces {
            let mut positions = Vec::new();
            let mut normals = Vec::new();
            let mut texcoords = Vec::new();
            let mut bone_ids = Vec::new();
            let mut weights = Vec::new();

            let material = &material_set.materials[surface.material_index as usize];
            let batches = surface.display_list.parse::<SkinnedVertexDescriptor>(
                material.vertex_attr_flags,
                &cmdl.position_data,
                &cmdl.normal_data,
                &cmdl.uv_float_data,
                &vertex_bone_ids,
                &vertex_weights,
            )?;
            for batch in batches {
                positions.extend_from_slice(&batch.positions);
                normals.extend_from_slice(&batch.normals);
                texcoords.extend_from_slice(&batch.texcoords);
                bone_ids.extend_from_slice(&batch.bone_ids);
                weights.extend_from_slice(&batch.weights);
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
                bone_ids,
                weights,
            });
        }

        Ok(Self {
            skin: Some(CanonicalMeshSkin { skeleton, skin }),
            surfaces,
            texture_ids: material_set.texture_ids.clone(),
        })
    }
}

fn interpret_bone(cinf: &Cinf, bone_id: u32) -> CanonicalMeshBone {
    let bone = cinf.bones.iter().find(|x| x.bone_id == bone_id).unwrap();
    let name = cinf
        .bone_names
        .iter()
        .find(|x| x.id == bone_id)
        .unwrap()
        .name
        .clone();
    let mut children = Vec::new();
    for linked_bone_id in bone.linked_bones.iter().copied() {
        if cinf
            .bones
            .iter()
            .find(|x| x.bone_id == linked_bone_id && x.parent_bone_id == bone_id)
            .is_some()
        {
            children.push(interpret_bone(cinf, linked_bone_id));
        }
    }
    CanonicalMeshBone {
        name,
        id: bone_id,
        position: bone.position,
        children,
    }
}

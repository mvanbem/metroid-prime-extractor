use std::io::Read;

use anyhow::Result;
use gamecube::bytes::{ReadAsciiCStringExt, ReadFrom};
use gamecube::{ReadBytesExt, ReadTypedExt};

#[derive(Clone, Debug)]
pub struct Cinf {
    pub bones: Vec<Bone>,
    pub build_order_ids: Vec<u32>,
    pub bone_names: Vec<BoneName>,
}

impl ReadFrom for Cinf {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let bone_count = r.read_u32()?;
        let mut bones = Vec::new();
        for _ in 0..bone_count {
            bones.push(r.read_typed()?);
        }

        let build_order_id_count = r.read_u32()?;
        let mut build_order_ids = Vec::new();
        for _ in 0..build_order_id_count {
            build_order_ids.push(r.read_u32()?);
        }

        let bone_name_count = r.read_u32()?;
        let mut bone_names = Vec::new();
        for _ in 0..bone_name_count {
            bone_names.push(r.read_typed()?);
        }

        Ok(Self {
            bones,
            build_order_ids,
            bone_names,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Bone {
    pub bone_id: u32,
    pub parent_bone_id: u32,
    pub position: [f32; 3],
    pub linked_bones: Vec<u32>,
}

impl ReadFrom for Bone {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let bone_id = r.read_u32()?;
        let parent_bone_id = r.read_u32()?;

        let position_x = f32::from_bits(r.read_u32()?);
        let position_y = f32::from_bits(r.read_u32()?);
        let position_z = f32::from_bits(r.read_u32()?);
        let position = [position_x, position_y, position_z];

        let linked_bone_count = r.read_u32()?;
        let mut linked_bones = Vec::new();
        for _ in 0..linked_bone_count {
            linked_bones.push(r.read_u32()?);
        }

        Ok(Self {
            bone_id,
            parent_bone_id,
            position,
            linked_bones,
        })
    }
}

#[derive(Clone, Debug)]
pub struct BoneName {
    pub name: String,
    pub id: u32,
}

impl ReadFrom for BoneName {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let name = r.read_ascii_c_string()?;
        let id = r.read_u32()?;

        Ok(Self { name, id })
    }
}

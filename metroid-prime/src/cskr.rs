use std::io::Read;

use anyhow::Result;
use gamecube::bytes::ReadFrom;
use gamecube::{ReadBytesExt, ReadTypedExt};

#[derive(Clone, Debug)]
pub struct Cskr {
    pub vertex_groups: Vec<VertexGroup>,
}

impl ReadFrom for Cskr {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let weight_count = r.read_u32()?;

        let mut vertex_groups = Vec::new();
        let mut total_weights = 0;
        while total_weights < weight_count {
            let vertex_group: VertexGroup = r.read_typed()?;
            total_weights += vertex_group.weights.len() as u32;
            vertex_groups.push(vertex_group);
        }
        assert_eq!(total_weights, weight_count);

        Ok(Self { vertex_groups })
    }
}

#[derive(Clone, Debug)]
pub struct VertexGroup {
    pub weights: Vec<Weight>,
    pub vertex_count: u32,
}

impl ReadFrom for VertexGroup {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let weight_count = r.read_u32()?;
        let mut weights = Vec::new();
        for _ in 0..weight_count {
            weights.push(r.read_typed()?);
        }

        let vertex_count = r.read_u32()?;

        Ok(Self {
            weights,
            vertex_count,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Weight {
    pub bone_id: u32,
    pub weight: f32,
}

impl ReadFrom for Weight {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let bone_id = r.read_u32()?;
        let weight = f32::from_bits(r.read_u32()?);

        Ok(Self { bone_id, weight })
    }
}

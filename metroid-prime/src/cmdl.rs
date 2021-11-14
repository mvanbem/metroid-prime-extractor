use std::collections::VecDeque;
use std::io::Read;

use anyhow::Result;
use gamecube::bytes::ReadFrom;
use gamecube::{ReadBytesExt, ReadTypedExt};

use crate::gx::DisplayList;

pub struct Cmdl {
    pub flags: u32,
    pub x_min: f32,
    pub y_min: f32,
    pub z_min: f32,
    pub x_max: f32,
    pub y_max: f32,
    pub z_max: f32,
    pub materials: Vec<MaterialSet>,
    pub position_data: Vec<u8>,
    pub normal_data: Vec<u8>,
    pub color_data: Vec<u8>,
    pub uv_float_data: Vec<u8>,
    pub uv_short_data: Vec<u8>,
    pub surfaces: Vec<Surface>,
}

impl ReadFrom for Cmdl {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let magic = r.read_u32()?;
        assert_eq!(magic, 0xdeadbabe);
        let version = r.read_u32()?;
        assert_eq!(version, 2);
        let flags = r.read_u32()?;
        let x_min = f32::from_bits(r.read_u32()?);
        let y_min = f32::from_bits(r.read_u32()?);
        let z_min = f32::from_bits(r.read_u32()?);
        let x_max = f32::from_bits(r.read_u32()?);
        let y_max = f32::from_bits(r.read_u32()?);
        let z_max = f32::from_bits(r.read_u32()?);

        let section_count = r.read_u32()?;
        let material_set_count = r.read_u32()?;
        let mut section_sizes = Vec::new();
        for _ in 0..section_count {
            section_sizes.push(r.read_u32()?);
        }

        // Pad the read header to a 32 byte boundary.
        let header_size = 0x2c + 4 * section_count as usize;
        let remainder = header_size & 31;
        if remainder > 0 {
            let mut buf = [0; 31];
            r.read_exact(&mut buf[..32 - remainder])?;
        }

        let mut sections = VecDeque::new();
        for size in section_sizes {
            let padded_size = (size + 31) & !31;
            let mut data = vec![0; padded_size as usize];
            r.read_exact(&mut data)?;
            data.resize(size as usize, 0);
            sections.push_back(data);
        }

        let mut materials = Vec::new();
        for _ in 0..material_set_count {
            let data = sections.pop_front().unwrap();
            materials.push(data.as_slice().read_typed()?);
        }

        let position_data = sections.pop_front().unwrap();
        let normal_data = sections.pop_front().unwrap();
        let color_data = sections.pop_front().unwrap();
        let uv_float_data = sections.pop_front().unwrap();
        let uv_short_data = if flags & 4 != 0 {
            sections.pop_front().unwrap()
        } else {
            Vec::new()
        };

        let surface_end_offsets = {
            let data = sections.pop_front().unwrap();
            let mut r = data.as_slice();
            let mut surface_end_offsets = Vec::new();
            let count = r.read_u32()?;
            for _ in 0..count {
                surface_end_offsets.push(r.read_u32()?);
            }
            surface_end_offsets
        };

        let mut surfaces = Vec::new();
        for _ in surface_end_offsets {
            let data = sections.pop_front().unwrap();
            surfaces.push(data.as_slice().read_typed()?);
        }

        Ok(Cmdl {
            flags,
            x_min,
            y_min,
            z_min,
            x_max,
            y_max,
            z_max,
            materials,
            position_data,
            normal_data,
            color_data,
            uv_float_data,
            uv_short_data,
            surfaces,
        })
    }
}

pub struct MaterialSet {
    pub texture_ids: Vec<u32>,
    pub materials: Vec<Material>,
}

impl ReadFrom for MaterialSet {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let texture_count = r.read_u32()?;
        let mut texture_ids = Vec::new();
        for _ in 0..texture_count {
            texture_ids.push(r.read_u32()?);
        }

        let material_count = r.read_u32()?;
        let mut material_end_offsets = Vec::new();
        for _ in 0..material_count {
            material_end_offsets.push(r.read_u32()?);
        }

        let mut last_end_offset = 0;
        let mut materials = Vec::new();
        for end_offset in material_end_offsets {
            let len = end_offset - last_end_offset;
            last_end_offset = end_offset;
            let mut buf = vec![0; len as usize];
            r.read_exact(&mut buf)?;
            materials.push(buf.as_slice().read_typed()?);
        }

        Ok(Self {
            texture_ids,
            materials,
        })
    }
}

pub struct Material {
    pub flags: u32,
    pub texture_indices: Vec<u32>,
    pub vertex_attr_flags: u32,
    pub group_index: u32,
    pub konsts: Vec<u32>,
    pub blend_dst_factor: u16,
    pub blend_src_factor: u16,
    pub reflection_indirect_texture_slot: Option<u32>,
    pub color_channel_flags: Vec<u32>,
    pub tev_stages: Vec<TevStage>,
    pub tev_texture_inputs: Vec<TevTextureInput>,
    pub tev_texgen_flags: Vec<u32>,
}

impl ReadFrom for Material {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let flags = r.read_u32()?;
        let texture_count = r.read_u32()?;
        let mut texture_indices = Vec::new();
        for _ in 0..texture_count {
            texture_indices.push(r.read_u32()?);
        }

        let vertex_attr_flags = r.read_u32()?;
        let group_index = r.read_u32()?;

        let mut konsts = Vec::new();
        if (flags & 0x8) != 0 {
            let konst_count = r.read_u32()?;
            for _ in 0..konst_count {
                konsts.push(r.read_u32()?);
            }
        }

        let blend_dst_factor = r.read_u16()?;
        let blend_src_factor = r.read_u16()?;

        let mut reflection_indirect_texture_slot = None;
        if (flags & 0x400) != 0 {
            reflection_indirect_texture_slot = Some(r.read_u32()?);
        }

        let color_channel_count = r.read_u32()?;
        let mut color_channel_flags = Vec::new();
        for _ in 0..color_channel_count {
            color_channel_flags.push(r.read_u32()?);
        }

        let tev_stage_count = r.read_u32()?;
        let mut tev_stages = Vec::new();
        for _ in 0..tev_stage_count {
            tev_stages.push(r.read_typed()?);
        }
        let mut tev_texture_inputs = Vec::new();
        for _ in 0..tev_stage_count {
            tev_texture_inputs.push(r.read_typed()?);
        }

        let texgen_count = r.read_u32()?;
        let mut tev_texgen_flags = Vec::new();
        for _ in 0..texgen_count {
            tev_texgen_flags.push(r.read_typed()?);
        }

        // TODO: UV Animations

        Ok(Self {
            flags,
            texture_indices,
            vertex_attr_flags,
            group_index,
            konsts,
            blend_dst_factor,
            blend_src_factor,
            reflection_indirect_texture_slot,
            color_channel_flags,
            tev_stages,
            tev_texture_inputs,
            tev_texgen_flags,
        })
    }
}

pub struct TevStage {
    pub color_in: u32,
    pub alpha_in: u32,
    pub color_op: u32,
    pub alpha_op: u32,
    pub alpha_konst: u8,
    pub color_konst: u8,
    pub rasterized_color: u8,
}

impl ReadFrom for TevStage {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let color_in = r.read_u32()?;
        let alpha_in = r.read_u32()?;
        let color_op = r.read_u32()?;
        let alpha_op = r.read_u32()?;
        let _padding = r.read_u8()?;
        let alpha_konst = r.read_u8()?;
        let color_konst = r.read_u8()?;
        let rasterized_color = r.read_u8()?;
        Ok(Self {
            color_in,
            alpha_in,
            color_op,
            alpha_op,
            alpha_konst,
            color_konst,
            rasterized_color,
        })
    }
}

pub struct TevTextureInput {
    pub texture_tev_input: u8,
    pub tex_coord_tev_input: u8,
}

impl ReadFrom for TevTextureInput {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let _ = r.read_u16()?;
        let texture_tev_input = r.read_u8()?;
        let tex_coord_tev_input = r.read_u8()?;
        Ok(Self {
            texture_tev_input,
            tex_coord_tev_input,
        })
    }
}

pub struct Surface {
    pub center: [f32; 3],
    pub material_index: u32,
    pub normal_divisor: u16,
    pub reflective_normal: [f32; 3],
    pub display_list: DisplayList,
}

impl ReadFrom for Surface {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let center_x = f32::from_bits(r.read_u32()?);
        let center_y = f32::from_bits(r.read_u32()?);
        let center_z = f32::from_bits(r.read_u32()?);
        let material_index = r.read_u32()?;
        let normal_divisor = r.read_u16()?;
        let _untrustworthy_display_list_size = r.read_u16()?;
        let _placeholder = r.read_u32()?;
        let _placeholder = r.read_u32()?;
        let extra_data_size = r.read_u32()?;
        let reflective_normal_x = f32::from_bits(r.read_u32()?);
        let reflective_normal_y = f32::from_bits(r.read_u32()?);
        let reflective_normal_z = f32::from_bits(r.read_u32()?);
        let _unused = r.read_u16()?;
        let _unused = r.read_u16()?;

        // Read and discard the extra data.
        {
            let mut buf = vec![0; extra_data_size as usize];
            r.read_exact(&mut buf)?;
        }

        // Pad the read header to a 32 byte boundary.
        let header_size = 0x30 + extra_data_size as usize;
        let remainder = header_size & 31;
        if remainder > 0 {
            let mut buf = [0; 31];
            r.read_exact(&mut buf[..32 - remainder])?;
        }

        // The remainder of the surface section is a GX display list.
        let display_list = r.read_typed()?;

        Ok(Self {
            center: [center_x, center_y, center_z],
            material_index,
            normal_divisor,
            reflective_normal: [
                reflective_normal_x,
                reflective_normal_y,
                reflective_normal_z,
            ],
            display_list,
        })
    }
}

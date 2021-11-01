use std::io::Read;

use anyhow::Result;

use crate::bytes::ReadFrom;
use crate::{ReadArrayExt, ReadBytesExt};

#[derive(Clone)]
pub struct Dol {
    section_offsets: [u32; 18],
    section_load_addrs: [u32; 18],
    section_sizes: [u32; 18],
    entry_point: u32,
}

impl Dol {}

impl ReadFrom for Dol {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let section_offsets = r.read_array()?;
        let section_load_addrs = r.read_array()?;
        let section_sizes = r.read_array()?;
        r.read_u32()?;
        r.read_u32()?;
        let entry_point = r.read_u32()?;

        Ok(Self {
            section_offsets,
            section_load_addrs,
            section_sizes,
            entry_point,
        })
    }
}

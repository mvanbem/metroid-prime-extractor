use anyhow::{bail, Result};
use flate2::{Decompress, FlushDecompress};
use gamecube::bytes::ReadFixedCapacityAsciiCStringExt;
use gamecube::ReadBytesExt;

pub struct Pak<'a> {
    name_table: Vec<NameTableEntry>,
    resource_table: Vec<ResourceTableEntry<'a>>,
}

impl<'a> Pak<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self> {
        let mut r = data;
        let version = r.read_u32()?;
        assert_eq!(version, 0x00030005);
        let reserved = r.read_u32()?;
        assert_eq!(reserved, 0);

        let name_count = r.read_u32()?;
        let mut name_table = Vec::new();
        for _ in 0..name_count {
            let fourcc = r.read_fixed_capacity_ascii_c_string(4)?;
            let file_id = r.read_u32()?;
            let name_len = r.read_u32()?;
            let name = std::str::from_utf8(&r[..name_len as usize])?.to_string();
            r = &r[name_len as usize..];
            name_table.push(NameTableEntry {
                fourcc,
                file_id,
                name,
            });
        }

        let resource_count = r.read_u32()?;
        let mut resource_table = Vec::new();
        for _ in 0..resource_count {
            let compression = r.read_u32()?;
            let fourcc = r.read_fixed_capacity_ascii_c_string(4)?;
            let file_id = r.read_u32()?;
            let size = r.read_u32()?;
            let offset = r.read_u32()?;
            resource_table.push(ResourceTableEntry {
                compression,
                _fourcc: fourcc,
                file_id,
                data: &data[offset as usize..(offset + size) as usize],
            });
        }

        Ok(Self {
            name_table,
            resource_table,
        })
    }

    pub fn iter(&self) -> PakIter<'_> {
        PakIter {
            iter: self.name_table.iter(),
        }
    }

    pub fn entry(&self, name: &str) -> Option<&NameTableEntry> {
        self.name_table.iter().find(|entry| entry.name == name)
    }

    pub fn data(&self, file_id: u32) -> Result<Option<Vec<u8>>> {
        let resource = match self
            .resource_table
            .iter()
            .find(|entry| entry.file_id == file_id)
        {
            Some(entry) => entry,
            None => return Ok(None),
        };

        match resource.compression {
            0 => Ok(Some(resource.data.to_vec())),
            1 => {
                let uncompressed_size = resource.data.clone().read_u32()? as usize;
                let compressed = &resource.data[4..];

                let mut uncompressed = Vec::with_capacity(uncompressed_size);
                uncompressed.resize(uncompressed_size, 0);
                assert_eq!(
                    Decompress::new(true).decompress(
                        compressed,
                        &mut uncompressed,
                        FlushDecompress::Finish
                    )?,
                    flate2::Status::StreamEnd,
                );

                Ok(Some(uncompressed))
            }
            _ => bail!("Unexpected compression: {}", resource.compression),
        }
    }
}

pub struct PakIter<'a> {
    iter: std::slice::Iter<'a, NameTableEntry>,
}

impl<'a> Iterator for PakIter<'a> {
    type Item = NameTableEntry;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().cloned()
    }
}

impl<'a> IntoIterator for &'a Pak<'_> {
    type Item = NameTableEntry;

    type IntoIter = PakIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Clone)]
pub struct NameTableEntry {
    fourcc: String,
    file_id: u32,
    name: String,
}

impl NameTableEntry {
    pub fn fourcc(&self) -> &str {
        &self.fourcc
    }

    pub fn file_id(&self) -> u32 {
        self.file_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

struct ResourceTableEntry<'a> {
    compression: u32,
    _fourcc: String,
    file_id: u32,
    data: &'a [u8],
}

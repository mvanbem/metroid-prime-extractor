use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::bytes::{ReadAsciiCStringExt, ReadFixedCapacityAsciiCStringExt};
use crate::{Dol, ReadBytesExt, ReadTypedExt};

/// The size of a GameCube disc image.
pub const SIZE: u32 = 1459978240;

#[derive(Clone)]
pub struct Header {
    game_code: String,
    maker_code: String,
    disc_id: u8,
    version: u8,
}

impl Header {
    pub const SIZE: u32 = 8;

    pub fn new(mut data: &[u8]) -> Result<Self> {
        let game_code = data.read_fixed_capacity_ascii_c_string(4)?;
        let maker_code = data.read_fixed_capacity_ascii_c_string(2)?;
        let disc_id = data.read_u8()?;
        let version = data.read_u8()?;

        Ok(Self {
            game_code,
            maker_code,
            disc_id,
            version,
        })
    }

    pub fn game_code(&self) -> &str {
        &self.game_code
    }

    pub fn maker_code(&self) -> &str {
        &self.maker_code
    }

    pub fn disc_id(&self) -> u8 {
        self.disc_id
    }

    pub fn version(&self) -> u8 {
        self.version
    }
}

#[derive(Clone)]
pub struct Disc<'a> {
    data: &'a [u8],
    header: Header,
    main_executable: Dol,
    file_table: &'a [u8],
    root_entry_count: u32,
    string_table: &'a [u8],
}

impl<'a> Disc<'a> {
    const HEADER_OFFSET: usize = 0;
    const MAIN_EXECUTABLE_OFFSET: usize = 0x420;
    const FILE_TABLE_PTR_OFFSET: usize = 0x424;
    const FILE_TABLE_SIZE_OFFSET: usize = 0x428;

    const ROOT_ENTRY_COUNT_OFFSET: usize = 8;
    const FILE_TABLE_ENTRY_SIZE: usize = 12;

    pub fn new(data: &'a [u8]) -> Result<Self> {
        let header = Header::new(&data[Self::HEADER_OFFSET..])?;
        let main_executable = (&data[Self::MAIN_EXECUTABLE_OFFSET..]).read_typed()?;
        let filesystem_table_ptr = (&data[Self::FILE_TABLE_PTR_OFFSET..]).read_u32()?;
        let filesystem_table_size = (&data[Self::FILE_TABLE_SIZE_OFFSET..]).read_u32()?;
        let filesystem_table = &data[filesystem_table_ptr as usize
            ..(filesystem_table_ptr + filesystem_table_size) as usize];

        let root_entry_count = (&filesystem_table[Self::ROOT_ENTRY_COUNT_OFFSET..]).read_u32()?;
        let string_table =
            &filesystem_table[root_entry_count as usize * Self::FILE_TABLE_ENTRY_SIZE..];

        Ok(Self {
            data,
            header,
            main_executable,
            file_table: filesystem_table,
            root_entry_count,
            string_table,
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn main_executable(&self) -> &Dol {
        &self.main_executable
    }

    pub fn iter_files(&self) -> impl Iterator<Item = Result<File>> + '_ {
        let mut r = &self.file_table[Self::FILE_TABLE_ENTRY_SIZE..];
        let mut path = PathBuf::new();
        let mut dir_ends = Vec::new();
        (1..self.root_entry_count).filter_map(move |index| {
            (|| {
                if dir_ends.last().copied() == Some(index) {
                    path.pop();
                    dir_ends.pop();
                }

                let entry = FileTableEntry::new(&mut r, self.string_table)?;
                match entry.data {
                    FileTableEntryData::File { offset, size } => {
                        let mut file_path = path.clone();
                        file_path.push(entry.name);
                        Ok(Some(File {
                            path: file_path,
                            data: &self.data[offset as usize..(offset + size) as usize],
                        }))
                    }
                    FileTableEntryData::Directory { end_index } => {
                        path.push(entry.name);
                        dir_ends.push(end_index);
                        Ok(None)
                    }
                }
            })()
            .transpose()
        })
    }

    pub fn find_file(&self, path: &Path) -> Result<Option<File>> {
        for file in self.iter_files() {
            let file = file?;
            if &file.path == path {
                return Ok(Some(file));
            }
        }
        Ok(None)
    }
}

struct FileTableEntry {
    name: String,
    data: FileTableEntryData,
}

enum FileTableEntryData {
    File { offset: u32, size: u32 },
    Directory { end_index: u32 },
}

impl FileTableEntry {
    fn new<R: Read>(mut r: R, string_table: &[u8]) -> Result<Self> {
        let tmp = r.read_u32()?;
        let flags = (tmp >> 24) as u8;
        let name_offset = tmp & 0x00ffffff;
        let name = (&string_table[name_offset as usize..]).read_ascii_c_string()?;
        let data = match flags {
            0 => {
                let offset = r.read_u32()?;
                let size = r.read_u32()?;
                FileTableEntryData::File { offset, size }
            }
            1 => {
                r.read_u32()?;
                let end_index = r.read_u32()?;
                FileTableEntryData::Directory { end_index }
            }
            _ => bail!("unexpected filesystem entry flags: 0x{:02x}", flags),
        };
        Ok(FileTableEntry { name, data })
    }
}

#[derive(Clone, Debug)]
pub struct File<'a> {
    path: PathBuf,
    data: &'a [u8],
}

impl<'a> File<'a> {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

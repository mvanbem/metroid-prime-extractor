use std::ffi::OsStr;
use std::fs::File;
use std::os::unix::prelude::AsRawFd;
use std::slice::from_raw_parts;

use anyhow::{bail, Result};
use gamecube::bytes::ReadFrom;
use gamecube::disc::Header;
use gamecube::{Disc, ReadTypedExt};
use mmap::{MapOption, MemoryMap};

use crate::ancs::Ancs;
use crate::pak::Pak;

mod ancs;
mod pak;

fn main() -> Result<()> {
    let disc_file = File::open("/home/mvanbem/Metroid Prime (USA) (v1.00).iso")?;
    let disc_mmap = MemoryMap::new(
        gamecube::disc::SIZE as usize,
        &[
            MapOption::MapFd(disc_file.as_raw_fd()),
            MapOption::MapReadable,
        ],
    )?;
    assert_eq!(disc_mmap.len(), gamecube::disc::SIZE as usize);
    let disc_data = unsafe { from_raw_parts(disc_mmap.data(), disc_mmap.len()) };
    let disc = Disc::new(disc_data)?;
    verify_disc(disc.header())?;

    let pak = Pak::new(
        disc.find_file("SamusGun.pak".as_ref())?
            .expect("Couldn't find SamusGun.pak")
            .data(),
    )?;
    let ancs: Ancs = pak
        .data(
            pak.entry("Plasma")
                .expect("Couldn't find Plasma resource")
                .file_id(),
        )?
        .unwrap()
        .as_slice()
        .read_typed()?;

    println!("{:#?}", ancs);

    // Attempt to parse every file with a known type.
    for file in disc.iter_files() {
        let file = file?;
        if file.path().extension().and_then(OsStr::to_str) == Some("pak") {
            let pak = Pak::new(file.data())?;
            for entry in &pak {
                let data = pak.data(entry.file_id())?.unwrap();
                let result = match entry.fourcc() {
                    "ANCS" => Ancs::read_from(&mut data.as_slice()).map(|_| ()),
                    _ => Ok(()),
                };
                match result {
                    Ok(()) => (),
                    Err(e) => {
                        println!(
                            "Error in {} {:>4} {}: {}",
                            file.path().display(),
                            entry.fourcc(),
                            entry.name(),
                            e,
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

fn verify_disc(header: &Header) -> Result<()> {
    if header.game_code() != "GM8E" {
        bail!(
            "Disc check: game code is {:?}, want \"GM8E\"",
            header.game_code()
        );
    }
    if header.maker_code() != "01" {
        bail!(
            "Disc check: maker code is {:?}, want \"01\"",
            header.maker_code()
        );
    }
    if header.disc_id() != 0 {
        bail!("Disc check: disc ID is {}, want 0", header.disc_id());
    }
    if header.version() != 0 {
        bail!("Disc check: game code is {}, want 0", header.version());
    }
    Ok(())
}

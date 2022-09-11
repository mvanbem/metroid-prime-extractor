#![allow(dead_code)]

use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use byteorder::{LittleEndian, WriteBytesExt};
use clap::Parser;
use gamecube::bytes::ReadFrom;
use gamecube::disc::Header;
use gamecube::{Disc, ReadTypedExt};
use gltf::Gltf;
use memmap::Mmap;
use nalgebra::Vector3;

use crate::ancs::Ancs;
use crate::cmdl::Cmdl;
use crate::mesh::CanonicalMesh;
use crate::pak::Pak;

mod ancs;
mod cmdl;
mod gx;
mod mesh;
mod pak;
mod txtr;

#[derive(Parser)]
struct Args {
    /// Path to a Metroid Prime disc image, USA version 1.0.
    image_path: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let disc_file = File::open(&args.image_path)?;
    let disc_mmap = unsafe { Mmap::map(&disc_file) }?;
    assert_eq!(disc_mmap.len(), gamecube::disc::SIZE as usize);

    let disc = Disc::new(&*disc_mmap)?;
    verify_disc(disc.header())?;

    let pak = Pak::new(
        disc.find_file("SamusGun.pak".as_ref())?
            .expect("Couldn't find SamusGun.pak")
            .data(),
    )?;
    let wave_ancs_pak_entry = pak.entry("Wave").expect("Couldn't find Wave resource");
    assert_eq!(wave_ancs_pak_entry.fourcc(), "ANCS");
    println!("SamusGun.pak/Wave");
    let wave_ancs: Ancs = pak
        .data(wave_ancs_pak_entry.file_id())?
        .unwrap()
        .as_slice()
        .read_typed()?;
    for wave_character in &wave_ancs.character_set.characters {
        if wave_character.name != "Wave" {
            continue;
        }
        println!("    Character: {}", wave_character.name);
        let wave_model_cmdl: Cmdl = pak
            .data(wave_character.model_id)?
            .unwrap()
            .as_slice()
            .read_typed()?;
        let mesh = CanonicalMesh::new(&pak, &wave_model_cmdl, 0)?;
        // export_collada(&mesh)?;
        export_gltf(&pak, &mesh)?;
    }

    Ok(())
}

fn process_all_resources(disc: &Disc) -> Result<()> {
    // Attempt to parse every file with a known type.
    for file in disc.iter_files() {
        let file = file?;
        if file.path().extension().and_then(OsStr::to_str) == Some("pak") {
            let pak = Pak::new(file.data())?;
            for entry in pak.iter_resources() {
                let name = pak
                    .iter_names()
                    .find(|e| e.file_id() == entry.file_id())
                    .map(|e| e.name().to_string());
                let data = pak.data(entry.file_id())?.unwrap();
                let result = match entry.fourcc() {
                    "ANCS" => Ancs::read_from(&mut data.as_slice()).map(drop),
                    "CMDL" => Cmdl::read_from(&mut data.as_slice()).map(drop),
                    "TXTR" => {
                        let mut dump_path = PathBuf::new();
                        dump_path.push("out");
                        match &name {
                            Some(name) => dump_path.push(format!(
                                "{} {}.png",
                                file.path().file_name().unwrap().to_str().unwrap(),
                                name,
                            )),
                            None => dump_path.push(format!(
                                "{} 0x{:08x}.png",
                                file.path().file_name().unwrap().to_str().unwrap(),
                                entry.file_id(),
                            )),
                        }

                        if !dump_path.exists() {
                            let mut buf = Vec::<u8>::new();
                            let result = txtr::dump(&data, &mut buf);
                            if result.is_ok() {
                                let mut w = BufWriter::new(File::create(dump_path)?);
                                w.write_all(&buf)?;
                                w.flush().unwrap();
                            }
                            result
                        } else {
                            Ok(())
                        }
                    }
                    _ => Ok(()),
                };
                match result {
                    Ok(()) => (),
                    Err(e) => {
                        println!(
                            "Error in {} {:>4} 0x{:08x} {:?}: {}",
                            file.path().display(),
                            entry.fourcc(),
                            entry.file_id(),
                            name,
                            e,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn export_gltf(pak: &Pak, mesh: &CanonicalMesh) -> Result<()> {
    let mut file = BufWriter::new(File::create("gltf_export.gltf")?);
    make_gltf_document(pak, mesh)?.to_writer_pretty(&mut file)?;
    file.flush()?;

    Ok(())
}

fn make_gltf_document(pak: &Pak, mesh: &CanonicalMesh) -> Result<Gltf> {
    const ATTRIBUTE_STRIDE: usize = 32;
    const POSITION_OFFSET: usize = 0;
    const NORMAL_OFFSET: usize = 12;
    const TEXCOORD0_OFFSET: usize = 24;

    // Export all referenced textures and build glTF materials that refer to them.
    let mut images = Vec::new();
    let mut textures = Vec::new();
    let mut materials = Vec::new();
    for (index, texture_id) in mesh.texture_ids.iter().copied().enumerate() {
        let filename = format!("gltf_export_{index:02}.png");

        // Export the texture to a file.
        let texture_data = pak
            .data(texture_id)?
            .ok_or_else(|| anyhow!("Texture 0x{texture_id:08x} not found"))?;
        let mut file = BufWriter::new(File::create(&filename)?);
        txtr::dump(&texture_data, &mut file)?;
        file.flush()?;
        drop(file);

        images.push(gltf::Image {
            uri: Some(filename),
            mime_type: None,
            buffer_view: None,
        });

        textures.push(gltf::Texture {
            sampler: Some(gltf::SamplerIndex(0)),
            source: Some(gltf::ImageIndex(index)),
        });

        materials.push(gltf::Material {
            pbr_metallic_roughness: Some(gltf::PbrMetallicRoughness {
                base_color_factor: None,
                base_color_texture: Some(gltf::TextureInfo {
                    index: gltf::TextureIndex(index),
                    tex_coord: Some(0),
                }),
                metallic_factor: Some(1.0),
                roughness_factor: Some(0.25),
                metallic_roughness_texture: None,
            }),
        });
    }

    // Process all surfaces into index and attribute buffers, generating glTF accessors and mesh
    // primitives that refer to them.
    let mut index_buffer = vec![];
    let mut attribute_buffer = vec![];
    let mut accessors = Vec::new();
    let mut mesh_primitives = Vec::new();
    for surface in &mesh.surfaces {
        assert_eq!(surface.positions.len(), surface.normals.len());
        assert_eq!(surface.positions.len(), surface.texcoords.len());

        let first_texture_index = surface.texture_indices[0];

        let index_byte_offset = index_buffer.len();
        let attribute_byte_offset = attribute_buffer.len();

        let mut count = 0;
        let mut min_position = Vector3::repeat(f32::INFINITY);
        let mut max_position = Vector3::repeat(f32::NEG_INFINITY);
        for ((position, normal), texcoord) in surface
            .positions
            .iter()
            .zip(surface.normals.iter())
            .zip(surface.texcoords.iter())
        {
            index_buffer.write_u16::<LittleEndian>(count as u16)?;
            attribute_buffer.write_f32::<LittleEndian>(position[0])?;
            attribute_buffer.write_f32::<LittleEndian>(position[1])?;
            attribute_buffer.write_f32::<LittleEndian>(position[2])?;
            attribute_buffer.write_f32::<LittleEndian>(normal[0])?;
            attribute_buffer.write_f32::<LittleEndian>(normal[1])?;
            attribute_buffer.write_f32::<LittleEndian>(normal[2])?;
            attribute_buffer.write_f32::<LittleEndian>(texcoord[0])?;
            attribute_buffer.write_f32::<LittleEndian>(texcoord[1])?;
            count += 1;
            min_position = min_position.inf(&(*position).into());
            max_position = max_position.sup(&(*position).into());
        }

        let accessor_base_index = accessors.len();
        accessors.push(gltf::Accessor {
            buffer_view: Some(gltf::BufferViewIndex(0)),
            byte_offset: index_byte_offset,
            type_: gltf::AccessorType::Scalar,
            component_type: gltf::AccessorComponentType::UnsignedShort,
            count,
            min: None,
            max: None,
        });
        accessors.push(gltf::Accessor {
            buffer_view: Some(gltf::BufferViewIndex(1)),
            byte_offset: attribute_byte_offset + POSITION_OFFSET,
            type_: gltf::AccessorType::Vec3,
            component_type: gltf::AccessorComponentType::Float,
            count,
            min: Some(min_position.iter().copied().collect()),
            max: Some(max_position.iter().copied().collect()),
        });
        accessors.push(gltf::Accessor {
            buffer_view: Some(gltf::BufferViewIndex(1)),
            byte_offset: attribute_byte_offset + NORMAL_OFFSET,
            type_: gltf::AccessorType::Vec3,
            component_type: gltf::AccessorComponentType::Float,
            count,
            min: None,
            max: None,
        });
        accessors.push(gltf::Accessor {
            buffer_view: Some(gltf::BufferViewIndex(1)),
            byte_offset: attribute_byte_offset + TEXCOORD0_OFFSET,
            type_: gltf::AccessorType::Vec2,
            component_type: gltf::AccessorComponentType::Float,
            count,
            min: None,
            max: None,
        });

        mesh_primitives.push(gltf::MeshPrimitive {
            mode: gltf::MeshPrimitiveMode::Triangles,
            indices: gltf::AccessorIndex(accessor_base_index + 0),
            attributes: [
                (
                    gltf::MeshAttribute::Position,
                    gltf::AccessorIndex(accessor_base_index + 1),
                ),
                (
                    gltf::MeshAttribute::Normal,
                    gltf::AccessorIndex(accessor_base_index + 2),
                ),
                (
                    gltf::MeshAttribute::Texcoord(0),
                    gltf::AccessorIndex(accessor_base_index + 3),
                ),
            ]
            .into_iter()
            .collect(),
            material: Some(gltf::MaterialIndex(first_texture_index)),
        });
    }

    // Write out the index and attribute buffers to a single externally referenced file.
    let mut buffer_file = BufWriter::new(File::create("gltf_export.bin")?);
    buffer_file.write_all(&index_buffer)?;
    buffer_file.write_all(&attribute_buffer)?;
    buffer_file.flush()?;
    drop(buffer_file);

    // Build the rest of the glTF file.
    Ok(Gltf {
        accessors,
        asset: gltf::Asset {
            version: gltf::Version,
        },
        buffers: vec![gltf::Buffer {
            byte_length: index_buffer.len() + attribute_buffer.len(),
            uri: "gltf_export.bin".to_string(),
        }],
        buffer_views: vec![
            gltf::BufferView {
                buffer: gltf::BufferIndex(0),
                byte_offset: 0,
                byte_length: index_buffer.len(),
                byte_stride: None,
            },
            gltf::BufferView {
                buffer: gltf::BufferIndex(0),
                byte_offset: index_buffer.len(),
                byte_length: attribute_buffer.len(),
                byte_stride: Some(ATTRIBUTE_STRIDE),
            },
        ],
        images,
        materials,
        meshes: vec![gltf::Mesh {
            primitives: mesh_primitives,
        }],
        nodes: vec![gltf::Node {
            name: "object".to_string(),
            mesh: Some(gltf::MeshIndex(0)),
            ..Default::default()
        }],
        samplers: vec![gltf::Sampler {
            mag_filter: gltf::SamplerMagFilter::Linear,
            min_filter: gltf::SamplerMinFilter::LinearMipmapLinear,
            wrap_s: gltf::SamplerWrap::Repeat,
            wrap_t: gltf::SamplerWrap::Repeat,
        }],
        scene: Some(gltf::SceneIndex(0)),
        scenes: vec![gltf::Scene {
            name: "scene".to_string(),
            nodes: vec![gltf::NodeIndex(0)],
            ..Default::default()
        }],
        textures,
    })
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

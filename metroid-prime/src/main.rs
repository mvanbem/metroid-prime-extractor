#![allow(dead_code)]

use std::collections::HashMap;
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
use nalgebra::{Isometry3, UnitQuaternion, Vector3};

use crate::ancs::Ancs;
use crate::cmdl::Cmdl;
use crate::mesh::CanonicalMesh;
use crate::pak::{Pak, PakCache};

mod ancs;
mod cinf;
mod cmdl;
mod cskr;
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

    let mut pak = PakCache::new(Pak::new(
        disc.find_file("SamusGun.pak".as_ref())?
            .expect("Couldn't find SamusGun.pak")
            .data(),
    )?);
    let wave_ancs_pak_entry = pak.entry("Wave").expect("Couldn't find Wave resource");
    println!("SamusGun.pak/Wave");
    let wave_ancs: Ancs = pak
        .data_with_fourcc(wave_ancs_pak_entry.file_id(), "ANCS")?
        .unwrap()
        .as_slice()
        .read_typed()?;
    for (character_index, wave_character) in wave_ancs.character_set.characters.iter().enumerate() {
        if wave_character.name != "Wave" {
            continue;
        }
        println!("    Character: {}", wave_character.name);
        let mesh = CanonicalMesh::new(&mut pak, &wave_ancs, character_index, 0)?;
        export_gltf(&mut pak, &mesh)?;
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

fn export_gltf(pak: &mut PakCache, mesh: &CanonicalMesh) -> Result<()> {
    let mut file = BufWriter::new(File::create("gltf_export.gltf")?);
    make_gltf_document(pak, mesh)?.to_writer_pretty(&mut file)?;
    file.flush()?;

    Ok(())
}

fn make_gltf_document(pak: &mut PakCache, mesh: &CanonicalMesh) -> Result<Gltf> {
    const ATTRIBUTE_STRIDE: usize = 52;
    const POSITION_OFFSET: usize = 0;
    const NORMAL_OFFSET: usize = 12;
    const TEXCOORD0_OFFSET: usize = 24;
    const JOINTS0_OFFSET: usize = 32;
    const WEIGHTS0_OFFSET: usize = 36;

    // Export all referenced textures and build glTF materials that refer to them.
    let mut images = Vec::new();
    let mut textures = Vec::new();
    let mut materials = Vec::new();
    for (index, texture_id) in mesh.texture_ids.iter().copied().enumerate() {
        let filename = format!("gltf_export_{index:02}.png");

        // Export the texture to a file.
        let texture_data = pak
            .data_with_fourcc(texture_id, "TXTR")?
            .ok_or_else(|| anyhow!("Texture 0x{texture_id:08x} not found"))?;
        let mut file = BufWriter::new(File::create(&filename)?);
        txtr::dump(texture_data.as_slice(), &mut file)?;
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

    let mut nodes = Vec::new();
    let mut joints = Vec::new();
    let mut joints_by_bone_id = HashMap::new();
    let skeleton_root_node_index = extract_nodes_from_bone(
        &mut nodes,
        &mut joints,
        &mut joints_by_bone_id,
        Vector3::zeros(),
        &mesh.skeleton,
    );
    let mut inverse_bind_pose_buffer = Vec::new();
    for node_index in &joints {
        let matrix = match nodes[node_index.0].transform {
            gltf::Transform::Decomposed {
                translation: Some(translation),
                ..
            } => Isometry3::from_parts(translation, UnitQuaternion::identity())
                .inverse()
                .to_matrix(),
            _ => unreachable!(),
        };
        for entry in &matrix {
            inverse_bind_pose_buffer.write_f32::<LittleEndian>(*entry)?;
        }
    }
    let skin = gltf::Skin {
        inverse_bind_matrices: Some(gltf::AccessorIndex(0)),
        skeleton: None,
        joints,
    };

    // Process all surfaces into index and attribute buffers, generating glTF accessors and mesh
    // primitives that refer to them.
    let mut index_buffer = Vec::new();
    let mut attribute_buffer = Vec::new();
    let mut accessors = vec![gltf::Accessor {
        buffer_view: Some(gltf::BufferViewIndex(2)),
        byte_offset: 0,
        type_: gltf::AccessorType::Mat4,
        component_type: gltf::AccessorComponentType::Float,
        count: inverse_bind_pose_buffer.len() / 64,
        min: None,
        max: None,
    }];
    let mut mesh_primitives = Vec::new();
    for surface in &mesh.surfaces {
        assert_eq!(surface.positions.len(), surface.normals.len());
        assert_eq!(surface.positions.len(), surface.texcoords.len());
        assert_eq!(surface.positions.len(), surface.bone_ids.len());
        assert_eq!(surface.positions.len(), surface.weights.len());

        let first_texture_index = surface.texture_indices[0];

        let index_byte_offset = index_buffer.len();
        let attribute_byte_offset = attribute_buffer.len();

        let mut count = 0;
        let mut min_position = Vector3::repeat(f32::INFINITY);
        let mut max_position = Vector3::repeat(f32::NEG_INFINITY);
        for ((((position, normal), texcoord), &bone_id), &weight) in surface
            .positions
            .iter()
            .zip(surface.normals.iter())
            .zip(surface.texcoords.iter())
            .zip(surface.bone_ids.iter())
            .zip(surface.weights.iter())
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
            attribute_buffer.write_u8(joints_by_bone_id[&bone_id])?;
            attribute_buffer.write_u8(0)?;
            attribute_buffer.write_u8(0)?;
            attribute_buffer.write_u8(0)?;
            attribute_buffer.write_f32::<LittleEndian>(weight)?;
            attribute_buffer.write_f32::<LittleEndian>(0.0)?;
            attribute_buffer.write_f32::<LittleEndian>(0.0)?;
            attribute_buffer.write_f32::<LittleEndian>(0.0)?;
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
        accessors.push(gltf::Accessor {
            buffer_view: Some(gltf::BufferViewIndex(1)),
            byte_offset: attribute_byte_offset + JOINTS0_OFFSET,
            type_: gltf::AccessorType::Vec4,
            component_type: gltf::AccessorComponentType::UnsignedByte,
            count,
            min: None,
            max: None,
        });
        accessors.push(gltf::Accessor {
            buffer_view: Some(gltf::BufferViewIndex(1)),
            byte_offset: attribute_byte_offset + WEIGHTS0_OFFSET,
            type_: gltf::AccessorType::Vec4,
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
                (
                    gltf::MeshAttribute::Joints(0),
                    gltf::AccessorIndex(accessor_base_index + 4),
                ),
                (
                    gltf::MeshAttribute::Weights(0),
                    gltf::AccessorIndex(accessor_base_index + 5),
                ),
            ]
            .into_iter()
            .collect(),
            material: Some(gltf::MaterialIndex(first_texture_index)),
        });
    }
    let mesh_node_index = gltf::NodeIndex(nodes.len());
    nodes.push(gltf::Node {
        name: "mesh".to_string(),
        mesh: Some(gltf::MeshIndex(0)),
        skin: Some(gltf::SkinIndex(0)),
        ..Default::default()
    });

    // Write out the index and attribute buffers to a single externally referenced file.
    let mut buffer_file = BufWriter::new(File::create("gltf_export.bin")?);
    buffer_file.write_all(&index_buffer)?;
    buffer_file.write_all(&attribute_buffer)?;
    buffer_file.write_all(&inverse_bind_pose_buffer)?;
    buffer_file.flush()?;
    drop(buffer_file);

    // Build the rest of the glTF file.
    Ok(Gltf {
        accessors,
        asset: gltf::Asset {
            version: gltf::Version,
        },
        buffers: vec![gltf::Buffer {
            byte_length: index_buffer.len()
                + attribute_buffer.len()
                + inverse_bind_pose_buffer.len(),
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
            gltf::BufferView {
                buffer: gltf::BufferIndex(0),
                byte_offset: index_buffer.len() + attribute_buffer.len(),
                byte_length: inverse_bind_pose_buffer.len(),
                byte_stride: None,
            },
        ],
        images,
        materials,
        meshes: vec![gltf::Mesh {
            primitives: mesh_primitives,
        }],
        nodes,
        samplers: vec![gltf::Sampler {
            mag_filter: gltf::SamplerMagFilter::Linear,
            min_filter: gltf::SamplerMinFilter::LinearMipmapLinear,
            wrap_s: gltf::SamplerWrap::Repeat,
            wrap_t: gltf::SamplerWrap::Repeat,
        }],
        scene: Some(gltf::SceneIndex(0)),
        scenes: vec![gltf::Scene {
            name: "scene".to_string(),
            nodes: vec![mesh_node_index, skeleton_root_node_index],
            ..Default::default()
        }],
        skins: vec![skin],
        textures,
    })
}

fn extract_nodes_from_bone(
    nodes: &mut Vec<gltf::Node>,
    joints: &mut Vec<gltf::NodeIndex>,
    joints_by_bone_id: &mut HashMap<u32, u8>,
    origin: Vector3<f32>,
    bone: &mesh::CanonicalBone,
) -> gltf::NodeIndex {
    let position = Vector3::from_column_slice(&bone.position).into();
    let children = bone
        .children
        .iter()
        .map(|x| extract_nodes_from_bone(nodes, joints, joints_by_bone_id, position, x))
        .collect();

    let index = gltf::NodeIndex(nodes.len());
    nodes.push(gltf::Node {
        name: bone.name.clone(),
        children,
        transform: gltf::Transform::Decomposed {
            translation: Some((position - origin).into()),
            rotation: None,
            scale: None,
        },
        mesh: None,
        skin: None,
    });

    let joint = joints.len();
    joints.push(index);
    joints_by_bone_id.insert(bone.id, joint.try_into().unwrap());

    index
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

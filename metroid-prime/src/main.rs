use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::os::unix::prelude::AsRawFd;
use std::path::PathBuf;
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
mod txtr;

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
    let plasma_animation_pak_entry = pak.entry("Plasma").expect("Couldn't find Plasma resource");
    assert_eq!(plasma_animation_pak_entry.fourcc(), "ANCS");
    let _ancs: Ancs = pak
        .data(plasma_animation_pak_entry.file_id())?
        .unwrap()
        .as_slice()
        .read_typed()?;

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
                    "ANCS" => Ancs::read_from(&mut data.as_slice()).map(|_| ()),
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

    // Emit a test COLLADA file.
    let mut file = BufWriter::new(File::create("collada_export.dae")?);
    let now = chrono::Local::now();
    dae_parser::Document {
        asset: dae_parser::Asset {
            contributor: vec![],
            created: now.to_rfc3339(),
            keywords: vec![],
            modified: now.to_rfc3339(),
            revision: None,
            subject: None,
            title: None,
            unit: dae_parser::Unit {
                name: None,
                meter: 1.0,
            },
            up_axis: dae_parser::UpAxis::YUp,
        },
        library: vec![
            make_geometry_library(),
            make_controller_library(),
            make_visual_scene_library(),
        ],
        scene: Some(make_scene()),
        extra: vec![],
    }
    .write_to(&mut file)
    .unwrap();
    file.flush()?;

    Ok(())
}

fn make_geometry_library() -> dae_parser::LibraryElement {
    dae_parser::LibraryElement::Geometries(dae_parser::Library {
        asset: None,
        items: vec![dae_parser::Geometry {
            id: Some("cube_geom".to_string()),
            name: Some("cube_geom".to_string()),
            asset: None,
            element: dae_parser::GeometryElement::Mesh(dae_parser::Mesh {
                convex: false,
                sources: vec![
                    dae_parser::Source {
                        id: Some("cube_pos".to_string()),
                        name: None,
                        asset: None,
                        array: Some(dae_parser::ArrayElement::Float(dae_parser::FloatArray {
                            id: Some("cube_pos_array".to_string()),
                            val: vec![
                                -0.5, 0.5, 0.5, 0.5, 0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5,
                                -0.5, 0.5, -0.5, 0.5, 0.5, -0.5, -0.5, -0.5, -0.5, 0.5, -0.5, -0.5,
                            ]
                            .into_boxed_slice(),
                        })),
                        accessor: dae_parser::Accessor {
                            source: dae_parser::Url::Fragment("cube_pos_array".to_string()),
                            count: 8,
                            offset: 0,
                            stride: 3,
                            param: vec![
                                dae_parser::Param {
                                    sid: None,
                                    name: Some("X".to_string()),
                                    ty: "float".to_string(),
                                    semantic: None,
                                },
                                dae_parser::Param {
                                    sid: None,
                                    name: Some("Y".to_string()),
                                    ty: "float".to_string(),
                                    semantic: None,
                                },
                                dae_parser::Param {
                                    sid: None,
                                    name: Some("Z".to_string()),
                                    ty: "float".to_string(),
                                    semantic: None,
                                },
                            ],
                        },
                        technique: vec![],
                    },
                    dae_parser::Source {
                        id: Some("cube_normal".to_string()),
                        name: None,
                        asset: None,
                        array: Some(dae_parser::ArrayElement::Float(dae_parser::FloatArray {
                            id: Some("cube_normal_array".to_string()),
                            val: vec![
                                1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0,
                                0.0, 1.0, 0.0, 0.0, -1.0,
                            ]
                            .into_boxed_slice(),
                        })),
                        accessor: dae_parser::Accessor {
                            source: dae_parser::Url::Fragment("cube_normal_array".to_string()),
                            count: 6,
                            offset: 0,
                            stride: 3,
                            param: vec![
                                dae_parser::Param {
                                    sid: None,
                                    name: Some("X".to_string()),
                                    ty: "float".to_string(),
                                    semantic: None,
                                },
                                dae_parser::Param {
                                    sid: None,
                                    name: Some("Y".to_string()),
                                    ty: "float".to_string(),
                                    semantic: None,
                                },
                                dae_parser::Param {
                                    sid: None,
                                    name: Some("Z".to_string()),
                                    ty: "float".to_string(),
                                    semantic: None,
                                },
                            ],
                        },
                        technique: vec![],
                    },
                ],
                vertices: Some(dae_parser::Vertices {
                    id: "cube_vtx".to_string(),
                    name: None,
                    inputs: vec![dae_parser::Input {
                        semantic: dae_parser::Semantic::Position,
                        source: dae_parser::Url::Fragment("cube_pos".to_string()),
                    }],
                    position: 0,
                    extra: vec![],
                }),
                elements: vec![dae_parser::Primitive::PolyList(dae_parser::PolyList {
                    name: None,
                    material: None,
                    count: 6,
                    inputs: dae_parser::InputList {
                        inputs: vec![
                            dae_parser::InputS {
                                input: dae_parser::Input {
                                    semantic: dae_parser::Semantic::Vertex,
                                    source: dae_parser::Url::Fragment("cube_vtx".to_string()),
                                },
                                offset: 0,
                                set: None,
                            },
                            dae_parser::InputS {
                                input: dae_parser::Input {
                                    semantic: dae_parser::Semantic::Normal,
                                    source: dae_parser::Url::Fragment("cube_normal".to_string()),
                                },
                                offset: 1,
                                set: None,
                            },
                        ],
                        depth: 2,
                    },
                    data: dae_parser::PolyListGeom {
                        vcount: vec![4, 4, 4, 4, 4, 4].into_boxed_slice(),
                        prim: vec![
                            0, 4, 2, 4, 3, 4, 1, 4, 0, 2, 1, 2, 5, 2, 4, 2, 6, 3, 7, 3, 3, 3, 2, 3,
                            0, 1, 4, 1, 6, 1, 2, 1, 3, 0, 7, 0, 5, 0, 1, 0, 5, 5, 7, 5, 6, 5, 4, 5,
                        ]
                        .into_boxed_slice(),
                    },
                    extra: vec![],
                })],
                extra: vec![],
            }),
            extra: vec![],
        }],
        extra: vec![],
    })
}

fn make_controller_library() -> dae_parser::LibraryElement {
    dae_parser::LibraryElement::Controllers(dae_parser::Library {
        asset: None,
        items: vec![make_controller(
            "cube_skin",
            "cube_geom",
            &["root_bone", "leaf_bone"],
            &[
                1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0,
            ],
        )],
        extra: vec![],
    })
}

fn make_controller(
    base_name: &str,
    skin_source_fragment: &str,
    bone_names: &[&str],
    weights: &[f32],
) -> dae_parser::Controller {
    let joints_id = format!("{}_joints", base_name);
    let joints_array_id = format!("{}_joints_array", base_name);
    let bind_poses_id = format!("{}_bind_poses", base_name);
    let bind_poses_array_id = format!("{}_bind_poses_array", base_name);
    let weights_id = format!("{}_weights", base_name);
    let weights_array_id = format!("{}_weights_array", base_name);

    dae_parser::Controller {
        id: Some(base_name.to_string()),
        name: Some("armature".to_string()),
        asset: None,
        element: dae_parser::ControlElement::Skin(dae_parser::Skin {
            source: dae_parser::UrlRef::new(dae_parser::Url::Fragment(
                skin_source_fragment.to_string(),
            )),
            bind_shape_matrix: Some(Box::new([
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ])),
            sources: vec![
                dae_parser::Source {
                    id: Some(joints_id.clone()),
                    name: None,
                    asset: None,
                    array: Some(dae_parser::ArrayElement::Name(dae_parser::NameArray {
                        id: Some(joints_array_id.clone()),
                        val: bone_names
                            .iter()
                            .map(|name| name.to_string())
                            .collect::<Vec<String>>()
                            .into_boxed_slice(),
                    })),
                    accessor: dae_parser::Accessor {
                        source: dae_parser::Url::Fragment(joints_array_id),
                        count: 1,
                        offset: 0,
                        stride: 1,
                        param: vec![dae_parser::Param {
                            sid: None,
                            name: Some("JOINT".to_string()),
                            ty: "name".to_string(),
                            semantic: None,
                        }],
                    },
                    technique: vec![],
                },
                dae_parser::Source {
                    id: Some(bind_poses_id.clone()),
                    name: None,
                    asset: None,
                    array: Some(dae_parser::ArrayElement::Float(dae_parser::FloatArray {
                        id: Some(bind_poses_array_id.clone()),
                        val: std::iter::repeat([
                            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                            0.0, 1.0,
                        ])
                        .take(bone_names.len())
                        .flatten()
                        .collect::<Vec<f32>>()
                        .into_boxed_slice(),
                    })),
                    accessor: dae_parser::Accessor {
                        source: dae_parser::Url::Fragment(bind_poses_array_id),
                        count: 1,
                        offset: 0,
                        stride: 16,
                        param: vec![dae_parser::Param {
                            sid: None,
                            name: Some("TRANSFORM".to_string()),
                            ty: "float4x4".to_string(),
                            semantic: None,
                        }],
                    },
                    technique: vec![],
                },
                dae_parser::Source {
                    id: Some(weights_id.clone()),
                    name: None,
                    asset: None,
                    array: Some(dae_parser::ArrayElement::Float(dae_parser::FloatArray {
                        id: Some(weights_array_id.clone()),
                        val: weights.to_vec().into_boxed_slice(),
                    })),
                    accessor: dae_parser::Accessor {
                        source: dae_parser::Url::Fragment(weights_array_id),
                        count: weights.len(),
                        offset: 0,
                        stride: 1,
                        param: vec![dae_parser::Param {
                            sid: None,
                            name: Some("WEIGHT".to_string()),
                            ty: "float".to_string(),
                            semantic: None,
                        }],
                    },
                    technique: vec![],
                },
            ],
            joints: dae_parser::Joints {
                inputs: vec![
                    dae_parser::Input {
                        semantic: dae_parser::Semantic::Joint,
                        source: dae_parser::Url::Fragment(joints_id.clone()),
                    },
                    dae_parser::Input {
                        semantic: dae_parser::Semantic::InvBindMatrix,
                        source: dae_parser::Url::Fragment(bind_poses_id),
                    },
                ],
                joint: 0,
                extra: vec![],
            },
            weights: dae_parser::VertexWeights {
                count: weights.len(),
                inputs: dae_parser::InputList {
                    inputs: vec![
                        dae_parser::InputS {
                            input: dae_parser::Input {
                                semantic: dae_parser::Semantic::Joint,
                                source: dae_parser::Url::Fragment(joints_id),
                            },
                            offset: 0,
                            set: None,
                        },
                        dae_parser::InputS {
                            input: dae_parser::Input {
                                semantic: dae_parser::Semantic::Weight,
                                source: dae_parser::Url::Fragment(weights_id),
                            },
                            offset: 1,
                            set: None,
                        },
                    ],
                    depth: 2,
                },
                joint: 0,
                vcount: std::iter::repeat(bone_names.len() as u32)
                    .take(weights.len() / bone_names.len())
                    .collect::<Vec<u32>>()
                    .into_boxed_slice(),
                prim: (0..weights.len())
                    .into_iter()
                    .map(|weight_index| {
                        let bone_index = weight_index as usize % bone_names.len();
                        [bone_index as i32, weight_index as i32]
                    })
                    .flatten()
                    .collect::<Vec<i32>>()
                    .into_boxed_slice(),
                extra: vec![],
            },
            extra: vec![],
        }),
        extra: vec![],
    }
}

fn make_visual_scene_library() -> dae_parser::LibraryElement {
    dae_parser::LibraryElement::VisualScenes(dae_parser::Library {
        asset: None,
        items: vec![dae_parser::VisualScene {
            id: Some("DefaultScene".to_string()),
            name: None,
            asset: None,
            nodes: vec![
                dae_parser::Node {
                    id: Some("armature".to_string()),
                    name: Some("armature".to_string()),
                    sid: None,
                    ty: dae_parser::NodeType::Node,
                    layer: vec![],
                    asset: None,
                    transforms: vec![dae_parser::Transform::Translate(dae_parser::Translate(
                        Box::new([0.0, 0.0, 0.0]),
                    ))],
                    instance_camera: vec![],
                    instance_controller: vec![],
                    instance_geometry: vec![],
                    instance_light: vec![],
                    instance_node: vec![],
                    children: vec![dae_parser::Node {
                        id: None,
                        name: Some("root bone".to_string()),
                        sid: Some("root_bone".to_string()),
                        ty: dae_parser::NodeType::Joint,
                        layer: vec![],
                        asset: None,
                        transforms: vec![dae_parser::Transform::Translate(dae_parser::Translate(
                            Box::new([0.0, 0.0, 0.0]),
                        ))],
                        instance_camera: vec![],
                        instance_controller: vec![],
                        instance_geometry: vec![],
                        instance_light: vec![],
                        instance_node: vec![],
                        children: vec![dae_parser::Node {
                            id: None,
                            name: Some("leaf bone".to_string()),
                            sid: Some("leaf_bone".to_string()),
                            ty: dae_parser::NodeType::Joint,
                            layer: vec![],
                            asset: None,
                            transforms: vec![dae_parser::Transform::Translate(
                                dae_parser::Translate(Box::new([0.0, 2.0, 0.0])),
                            )],
                            instance_camera: vec![],
                            instance_controller: vec![],
                            instance_geometry: vec![],
                            instance_light: vec![],
                            instance_node: vec![],
                            children: vec![],
                            extra: vec![],
                        }],
                        extra: vec![],
                    }],
                    extra: vec![],
                },
                make_cube_node("cube", "cube", "cube_skin", "armature", [0.0, 0.0, 0.0]),
            ],
            evaluate_scene: vec![],
            extra: vec![],
        }],
        extra: vec![],
    })
}

fn make_cube_node(
    id: &str,
    name: &str,
    controller_id: &str,
    root_bone_id: &str,
    translation: [f32; 3],
) -> dae_parser::Node {
    dae_parser::Node {
        id: Some(id.to_string()),
        name: Some(name.to_string()),
        sid: None,
        ty: dae_parser::NodeType::Node,
        layer: vec![],
        asset: None,
        transforms: vec![dae_parser::Transform::Translate(dae_parser::Translate(
            Box::new(translation),
        ))],
        instance_camera: vec![],
        instance_controller: vec![dae_parser::Instance {
            sid: None,
            url: dae_parser::UrlRef::new(dae_parser::Url::Fragment(controller_id.to_string())),
            name: None,
            data: dae_parser::InstanceControllerData {
                skeleton: vec![dae_parser::Url::Fragment(root_bone_id.to_string())],
                bind_material: None,
            },
            extra: vec![],
        }],
        instance_geometry: vec![],
        instance_light: vec![],
        instance_node: vec![],
        children: vec![],
        extra: vec![],
    }
}

fn make_scene() -> dae_parser::Scene {
    dae_parser::Scene {
        instance_physics_scene: vec![],
        instance_visual_scene: Some(dae_parser::Instance {
            sid: None,
            url: dae_parser::UrlRef::new(dae_parser::Url::Fragment("DefaultScene".to_string())),
            name: None,
            data: (),
            extra: vec![],
        }),
        extra: vec![],
    }
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

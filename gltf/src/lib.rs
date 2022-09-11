use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;

use nalgebra::{Matrix4, Scale3, Translation3, UnitQuaternion, Vector3, Vector4};
use serde::ser::{SerializeSeq, SerializeStruct};
use serde::{Serialize, Serializer};

struct GltfMatrix4<'a>(&'a Matrix4<f32>);

impl<'a> Serialize for GltfMatrix4<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_seq(Some(16))?;
        for entry in self.0 {
            s.serialize_element(entry)?;
        }
        s.end()
    }
}

struct GltfVector3<'a>(&'a Vector3<f32>);

impl<'a> Serialize for GltfVector3<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_seq(Some(16))?;
        for entry in self.0 {
            s.serialize_element(entry)?;
        }
        s.end()
    }
}

struct GltfVector4<'a>(&'a Vector4<f32>);

impl<'a> Serialize for GltfVector4<'a> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_seq(Some(16))?;
        for entry in self.0 {
            s.serialize_element(entry)?;
        }
        s.end()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct AccessorIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct BufferIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct BufferViewIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ImageIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct MaterialIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct MeshIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct NodeIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SamplerIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SceneIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct SkinIndex(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct TextureIndex(pub usize);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gltf {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub accessors: Vec<Accessor>,
    pub asset: Asset,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buffers: Vec<Buffer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buffer_views: Vec<BufferView>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<Material>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub meshes: Vec<Mesh>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub samplers: Vec<Sampler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<SceneIndex>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<Scene>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skins: Vec<Skin>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub textures: Vec<Texture>,
}

impl Gltf {
    pub fn to_writer(&self, w: impl Write) -> serde_json::Result<()> {
        serde_json::to_writer(w, self)
    }

    pub fn to_writer_pretty(&self, w: impl Write) -> serde_json::Result<()> {
        serde_json::to_writer_pretty(w, self)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_view: Option<BufferViewIndex>,
    pub byte_offset: usize,
    #[serde(rename = "type")]
    pub type_: AccessorType,
    pub component_type: AccessorComponentType,
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<Vec<f32>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccessorComponentType {
    Byte,
    UnsignedByte,
    Short,
    UnsignedShort,
    UnsignedInt,
    Float,
}

impl Serialize for AccessorComponentType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(match self {
            Self::Byte => 5120,
            Self::UnsignedByte => 5121,
            Self::Short => 5122,
            Self::UnsignedShort => 5123,
            Self::UnsignedInt => 5125,
            Self::Float => 5126,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccessorType {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat3,
    Mat4,
}

impl Serialize for AccessorType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::Scalar => "SCALAR",
            Self::Vec2 => "VEC2",
            Self::Vec3 => "VEC3",
            Self::Vec4 => "VEC4",
            Self::Mat2 => "MAT2",
            Self::Mat3 => "MAT3",
            Self::Mat4 => "MAT4",
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Asset {
    pub version: Version,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Buffer {
    pub byte_length: usize,
    pub uri: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferView {
    pub buffer: BufferIndex,
    pub byte_offset: usize,
    pub byte_length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_stride: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_view: Option<BufferViewIndex>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Material {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pbr_metallic_roughness: Option<PbrMetallicRoughness>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Mesh {
    pub primitives: Vec<MeshPrimitive>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshAttribute {
    Position,
    Normal,
    Tangent,
    Texcoord(usize),
    Color(usize),
    Joints(usize),
    Weights(usize),
}

impl Serialize for MeshAttribute {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&match self {
            Self::Position => Cow::Borrowed("POSITION"),
            Self::Normal => Cow::Borrowed("NORMAL"),
            Self::Tangent => Cow::Borrowed("TANGENT"),
            Self::Texcoord(n) => Cow::Owned(format!("TEXCOORD_{n}")),
            Self::Color(n) => Cow::Owned(format!("COLOR_{n}")),
            Self::Joints(n) => Cow::Owned(format!("JOINTS_{n}")),
            Self::Weights(n) => Cow::Owned(format!("WEIGHTS_{n}")),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct MeshPrimitive {
    pub mode: MeshPrimitiveMode,
    pub indices: AccessorIndex,
    pub attributes: HashMap<MeshAttribute, AccessorIndex>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub material: Option<MaterialIndex>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum MeshPrimitiveMode {
    Points,
    Lines,
    LineLoop,
    LineStrip,
    Triangles,
    TriangleStrip,
    TriangleFan,
}

impl Serialize for MeshPrimitiveMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(*self as u8)
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Node {
    pub name: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NodeIndex>,
    #[serde(flatten)]
    pub transform: Transform,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<MeshIndex>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin: Option<SkinIndex>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PbrMetallicRoughness {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_color_factor: Option<[f32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_color_texture: Option<TextureInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metallic_factor: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roughness_factor: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metallic_roughness_texture: Option<TextureInfo>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sampler {
    pub mag_filter: SamplerMagFilter,
    pub min_filter: SamplerMinFilter,
    pub wrap_s: SamplerWrap,
    pub wrap_t: SamplerWrap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SamplerMagFilter {
    Nearest,
    Linear,
}

impl Serialize for SamplerMagFilter {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(match self {
            Self::Nearest => 9728,
            Self::Linear => 9729,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SamplerMinFilter {
    Nearest,
    Linear,
    NearestMipmapNearest,
    LinearMipmapNearest,
    NearestMipmapLinear,
    LinearMipmapLinear,
}

impl Serialize for SamplerMinFilter {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(match self {
            Self::Nearest => 9728,
            Self::Linear => 9729,
            Self::NearestMipmapNearest => 9984,
            Self::LinearMipmapNearest => 9985,
            Self::NearestMipmapLinear => 9986,
            Self::LinearMipmapLinear => 9987,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SamplerWrap {
    ClampToEdge,
    MirroredRepeat,
    Repeat,
}

impl Serialize for SamplerWrap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(match self {
            Self::ClampToEdge => 33071,
            Self::MirroredRepeat => 33648,
            Self::Repeat => 10497,
        })
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Scene {
    pub name: String,
    pub nodes: Vec<NodeIndex>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverse_bind_matrices: Option<AccessorIndex>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skeleton: Option<NodeIndex>,
    pub joints: Vec<NodeIndex>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Texture {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampler: Option<SamplerIndex>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ImageIndex>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureInfo {
    pub index: TextureIndex,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tex_coord: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub enum Transform {
    Matrix(Matrix4<f32>),
    Decomposed {
        translation: Option<Translation3<f32>>,
        rotation: Option<UnitQuaternion<f32>>,
        scale: Option<Scale3<f32>>,
    },
}

impl Default for Transform {
    fn default() -> Self {
        Self::Decomposed {
            translation: None,
            rotation: None,
            scale: None,
        }
    }
}

impl Serialize for Transform {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Matrix(matrix) => {
                let mut s = serializer.serialize_struct("GltfTransform::Matrix", 1)?;
                s.serialize_field("matrix", &GltfMatrix4(matrix))?;
                s.end()
            }
            Self::Decomposed {
                translation,
                rotation,
                scale,
            } => {
                let count = translation.is_some() as usize
                    + rotation.is_some() as usize
                    + scale.is_some() as usize;
                let mut s = serializer.serialize_struct("GltfTransform::Decomposed", count)?;
                if let Some(translation) = translation {
                    s.serialize_field("translation", &GltfVector3(&translation.vector))?;
                }
                if let Some(rotation) = rotation {
                    s.serialize_field("rotation", &GltfVector4(&rotation.coords))?;
                }
                if let Some(scale) = scale {
                    s.serialize_field("scale", &GltfVector3(&scale.vector))?;
                }
                s.end()
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Version;

impl Serialize for Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

use std::io::Read;

use anyhow::{bail, Result};
use gamecube::bytes::ReadFrom;
use gamecube::ReadBytesExt;

pub trait VertexDescriptor {
    type Joints: VertexAttribute;
    type Weights: VertexAttribute;
}

pub trait VertexAttribute: Copy + Default + Sized {
    type Data: ?Sized;

    fn get(data: &Self::Data, index: usize) -> Self;
}

impl VertexAttribute for () {
    type Data = ();

    fn get(_data: &(), _index: usize) -> Self {}
}

impl VertexAttribute for u32 {
    type Data = [u32];

    fn get(data: &[u32], index: usize) -> Self {
        data[index]
    }
}

impl VertexAttribute for f32 {
    type Data = [f32];

    fn get(data: &[f32], index: usize) -> Self {
        data[index]
    }
}

pub struct StaticVertexDescriptor;

impl VertexDescriptor for StaticVertexDescriptor {
    type Joints = ();
    type Weights = ();
}

pub struct SkinnedVertexDescriptor;

impl VertexDescriptor for SkinnedVertexDescriptor {
    type Joints = u32;
    type Weights = f32;
}

#[derive(Debug)]
pub struct DisplayList {
    data: Vec<u8>,
}

impl DisplayList {
    pub fn parse<V>(
        &self,
        vertex_attr_flags: u32,
        position_data: &[u8],
        normal_data: &[u8],
        uv_float_data: &[u8],
        joints: &<V::Joints as VertexAttribute>::Data,
        weights: &<V::Weights as VertexAttribute>::Data,
    ) -> Result<Vec<Batch<V::Joints, V::Weights>>>
    where
        V: VertexDescriptor,
    {
        let mut r = self.data.as_slice();
        let mut batches = Vec::new();
        loop {
            let opcode = r.read_u8()?;
            if opcode == 0 {
                break;
            }
            let primitive_type = opcode & 0xf8;
            let vertex_format = opcode & 0x07;
            match primitive_type {
                0x90 => batches.push(Self::parse_batch(
                    &mut r,
                    vertex_format,
                    Triangles::new(),
                    vertex_attr_flags,
                    position_data,
                    normal_data,
                    uv_float_data,
                    joints,
                    weights,
                )?),
                0x98 => batches.push(Self::parse_batch(
                    &mut r,
                    vertex_format,
                    TriangleStrip::new(),
                    vertex_attr_flags,
                    position_data,
                    normal_data,
                    uv_float_data,
                    joints,
                    weights,
                )?),
                0xa0 => batches.push(Self::parse_batch(
                    &mut r,
                    vertex_format,
                    TriangleFan::new(),
                    vertex_attr_flags,
                    position_data,
                    normal_data,
                    uv_float_data,
                    joints,
                    weights,
                )?),
                _ => bail!("unexpected GX primitive type: 0x{:02x}", primitive_type),
            }
        }
        Ok(batches)
    }

    fn parse_batch<R, H, BoneId, Weight>(
        r: &mut R,
        vertex_format: u8,
        mut vertex_handler: H,
        vertex_attr_flags: u32,
        position_data: &[u8],
        normal_data: &[u8],
        uv_float_data: &[u8],
        bone_ids: &BoneId::Data,
        weights: &Weight::Data,
    ) -> Result<Batch<BoneId, Weight>>
    where
        R: Read,
        H: VertexHandler<BoneId, Weight>,
        BoneId: VertexAttribute,
        Weight: VertexAttribute,
    {
        assert!((0..=2).contains(&vertex_format));

        let count = r.read_u16()?;
        for _ in 0..count {
            // Position
            assert!((vertex_attr_flags & 0x3) != 0);
            let (position, bone_id, weight) = {
                let index = r.read_u16()?;
                let mut data = &position_data[index as usize * 12..];
                let x = f32::from_bits(data.read_u32()?);
                let y = f32::from_bits(data.read_u32()?);
                let z = f32::from_bits(data.read_u32()?);
                let bone_id = BoneId::get(bone_ids, index as usize);
                let weight = Weight::get(weights, index as usize);
                ([x, y, z], bone_id, weight)
            };
            // Normal
            assert!((vertex_attr_flags & 0xc) != 0);
            let normal = match vertex_format {
                0 => {
                    let index = r.read_u16()?;
                    let mut data = &normal_data[index as usize * 12..];
                    let x = f32::from_bits(data.read_u32()?);
                    let y = f32::from_bits(data.read_u32()?);
                    let z = f32::from_bits(data.read_u32()?);
                    [x, y, z]
                }
                1 | 2 => {
                    let index = r.read_u16()?;
                    let mut data = &normal_data[index as usize * 6..];
                    let x = data.read_i16()? as f32;
                    let y = data.read_i16()? as f32;
                    let z = data.read_i16()? as f32;
                    let s = (x * x + y * y + z * z).sqrt().recip();
                    [s * x, s * y, s * z]
                }
                _ => unreachable!(),
            };
            // Color 0
            if (vertex_attr_flags & 0x30) != 0 {
                unimplemented!("Vertex attribute: Color 0")
            }
            // Color 1
            if (vertex_attr_flags & 0xc0) != 0 {
                unimplemented!("Vertex attribute: Color 1")
            }
            // Tex 0
            let texcoord = if (vertex_attr_flags & 0x300) != 0 {
                match vertex_format {
                    0 | 1 => {
                        let index = r.read_u16()?;
                        let mut data = &uv_float_data[index as usize * 8..];
                        let s = f32::from_bits(data.read_u32()?);
                        let t = f32::from_bits(data.read_u32()?);
                        Some([s, t])
                    }
                    2 => unimplemented!(),
                    _ => unreachable!(),
                }
            } else {
                None
            };
            // Tex 1
            if (vertex_attr_flags & 0xc00) != 0 {
                let _index = r.read_u16()?;
                // TODO: Read and save the texture coordinate.
            }
            // Tex 2
            if (vertex_attr_flags & 0x3000) != 0 {
                let _index = r.read_u16()?;
                // TODO: Read and save the texture coordinate.
            }
            // Tex 3
            if (vertex_attr_flags & 0xc000) != 0 {
                unimplemented!("Vertex attribute: Tex 3")
            }
            // Tex 4
            if (vertex_attr_flags & 0x30000) != 0 {
                unimplemented!("Vertex attribute: Tex 4")
            }
            // Tex 5
            if (vertex_attr_flags & 0xc0000) != 0 {
                unimplemented!("Vertex attribute: Tex 5")
            }
            // Tex 6
            if (vertex_attr_flags & 0x300000) != 0 {
                unimplemented!("Vertex attribute: Tex 6")
            }

            vertex_handler.handle_vertex(position, normal, texcoord, bone_id, weight);
        }

        Ok(vertex_handler.finish())
    }
}

impl ReadFrom for DisplayList {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        let mut data = Vec::new();
        r.read_to_end(&mut data)?;
        Ok(Self { data })
    }
}

trait VertexHandler<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    fn handle_vertex(
        &mut self,
        position: [f32; 3],
        normal: [f32; 3],
        texcoord: Option<[f32; 2]>,
        bone_id: BoneId,
        weight: Weight,
    );
    fn finish(self) -> Batch<BoneId, Weight>;
}

struct Triangles<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    bone_ids: Vec<BoneId>,
    weights: Vec<Weight>,
    position_a: [f32; 3],
    position_b: [f32; 3],
    normal_a: [f32; 3],
    normal_b: [f32; 3],
    texcoord_a: [f32; 2],
    texcoord_b: [f32; 2],
    bone_id_a: BoneId,
    bone_id_b: BoneId,
    weight_a: Weight,
    weight_b: Weight,
    // 0: empty buffer
    // 1: one buffered vertex
    // 2: two buffered vertices
    state: u8,
}

impl<BoneId, Weight> Triangles<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Vec::new(),
            bone_ids: Vec::new(),
            weights: Vec::new(),
            position_a: [0.0; 3],
            position_b: [0.0; 3],
            normal_a: [0.0; 3],
            normal_b: [0.0; 3],
            texcoord_a: [0.0; 2],
            texcoord_b: [0.0; 2],
            bone_id_a: Default::default(),
            bone_id_b: Default::default(),
            weight_a: Default::default(),
            weight_b: Default::default(),
            state: 0,
        }
    }
}

impl<BoneId, Weight> VertexHandler<BoneId, Weight> for Triangles<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    fn handle_vertex(
        &mut self,
        position: [f32; 3],
        normal: [f32; 3],
        texcoord: Option<[f32; 2]>,
        bone_id: BoneId,
        weight: Weight,
    ) {
        match self.state {
            0 => {
                self.position_a = position;
                self.normal_a = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_a = texcoord;
                }
                self.bone_id_a = bone_id;
                self.weight_a = weight;
                self.state = 1;
            }
            1 => {
                self.position_b = position;
                self.normal_b = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_b = texcoord;
                }
                self.bone_id_b = bone_id;
                self.weight_b = weight;
                self.state = 2;
            }
            2 => {
                self.positions.push(self.position_a);
                self.positions.push(self.position_b);
                self.positions.push(position);
                self.normals.push(self.normal_a);
                self.normals.push(self.normal_b);
                self.normals.push(normal);
                if let Some(texcoord) = texcoord {
                    self.texcoords.push(self.texcoord_a);
                    self.texcoords.push(self.texcoord_b);
                    self.texcoords.push(texcoord);
                }
                self.bone_ids.push(self.bone_id_a);
                self.bone_ids.push(self.bone_id_b);
                self.bone_ids.push(bone_id);
                self.weights.push(self.weight_a);
                self.weights.push(self.weight_b);
                self.weights.push(weight);
                self.state = 0;
            }
            _ => unreachable!(),
        }
    }

    fn finish(self) -> Batch<BoneId, Weight> {
        assert_eq!(self.state, 0);
        Batch {
            positions: self.positions,
            normals: self.normals,
            texcoords: self.texcoords,
            bone_ids: self.bone_ids,
            weights: self.weights,
        }
    }
}

struct TriangleStrip<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    bone_ids: Vec<BoneId>,
    weights: Vec<Weight>,
    position_a: [f32; 3],
    position_b: [f32; 3],
    normal_a: [f32; 3],
    normal_b: [f32; 3],
    texcoord_a: [f32; 2],
    texcoord_b: [f32; 2],
    bone_id_a: BoneId,
    bone_id_b: BoneId,
    weight_a: Weight,
    weight_b: Weight,
    // 0: empty buffer
    // 1: one buffered vertex
    // 2: two buffered vertices, even parity
    // 3: two buffered vertices, odd parity
    state: u8,
}

impl<BoneId, Weight> TriangleStrip<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Vec::new(),
            bone_ids: Vec::new(),
            weights: Vec::new(),
            position_a: [0.0; 3],
            position_b: [0.0; 3],
            normal_a: [0.0; 3],
            normal_b: [0.0; 3],
            texcoord_a: [0.0; 2],
            texcoord_b: [0.0; 2],
            bone_id_a: Default::default(),
            bone_id_b: Default::default(),
            weight_a: Default::default(),
            weight_b: Default::default(),
            state: 0,
        }
    }

    fn shift(
        &mut self,
        position: [f32; 3],
        normal: [f32; 3],
        texcoord: Option<[f32; 2]>,
        bone_id: BoneId,
        weight: Weight,
    ) {
        self.position_a = self.position_b;
        self.normal_a = self.normal_b;
        self.texcoord_a = self.texcoord_b;
        self.bone_id_a = self.bone_id_b;
        self.weight_a = self.weight_b;

        self.position_b = position;
        self.normal_b = normal;
        if let Some(texcoord) = texcoord {
            self.texcoord_b = texcoord;
        }
        self.bone_id_b = bone_id;
        self.weight_b = weight;
    }
}

impl<BoneId, Weight> VertexHandler<BoneId, Weight> for TriangleStrip<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    fn handle_vertex(
        &mut self,
        position: [f32; 3],
        normal: [f32; 3],
        texcoord: Option<[f32; 2]>,
        bone_id: BoneId,
        weight: Weight,
    ) {
        match self.state {
            0 => {
                self.position_a = position;
                self.normal_a = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_a = texcoord;
                }
                self.bone_id_a = bone_id;
                self.weight_a = weight;
                self.state = 1;
            }
            1 => {
                self.position_b = position;
                self.normal_b = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_b = texcoord;
                }
                self.bone_id_b = bone_id;
                self.weight_b = weight;
                self.state = 2;
            }
            2 => {
                self.positions.push(self.position_a);
                self.positions.push(self.position_b);
                self.positions.push(position);
                self.normals.push(self.normal_a);
                self.normals.push(self.normal_b);
                self.normals.push(normal);
                if let Some(texcoord) = texcoord {
                    self.texcoords.push(self.texcoord_a);
                    self.texcoords.push(self.texcoord_b);
                    self.texcoords.push(texcoord);
                }
                self.bone_ids.push(self.bone_id_a);
                self.bone_ids.push(self.bone_id_b);
                self.bone_ids.push(bone_id);
                self.weights.push(self.weight_a);
                self.weights.push(self.weight_b);
                self.weights.push(weight);
                self.shift(position, normal, texcoord, bone_id, weight);
                self.state = 3;
            }
            3 => {
                self.positions.push(self.position_b);
                self.positions.push(self.position_a);
                self.positions.push(position);
                self.normals.push(self.normal_b);
                self.normals.push(self.normal_a);
                self.normals.push(normal);
                if let Some(texcoord) = texcoord {
                    self.texcoords.push(self.texcoord_b);
                    self.texcoords.push(self.texcoord_a);
                    self.texcoords.push(texcoord);
                }
                self.bone_ids.push(self.bone_id_b);
                self.bone_ids.push(self.bone_id_a);
                self.bone_ids.push(bone_id);
                self.weights.push(self.weight_b);
                self.weights.push(self.weight_a);
                self.weights.push(weight);
                self.shift(position, normal, texcoord, bone_id, weight);
                self.state = 2;
            }
            _ => unreachable!(),
        }
    }

    fn finish(self) -> Batch<BoneId, Weight> {
        Batch {
            positions: self.positions,
            normals: self.normals,
            texcoords: self.texcoords,
            bone_ids: self.bone_ids,
            weights: self.weights,
        }
    }
}

struct TriangleFan<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    bone_ids: Vec<BoneId>,
    weights: Vec<Weight>,
    position_a: [f32; 3],
    position_b: [f32; 3],
    normal_a: [f32; 3],
    normal_b: [f32; 3],
    texcoord_a: [f32; 2],
    texcoord_b: [f32; 2],
    bone_id_a: BoneId,
    bone_id_b: BoneId,
    weight_a: Weight,
    weight_b: Weight,
    // 0: empty buffer
    // 1: one buffered vertex
    // 2: two buffered vertices
    state: u8,
}

impl<BoneId, Weight> TriangleFan<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Vec::new(),
            bone_ids: Vec::new(),
            weights: Vec::new(),
            position_a: [0.0; 3],
            position_b: [0.0; 3],
            normal_a: [0.0; 3],
            normal_b: [0.0; 3],
            texcoord_a: [0.0; 2],
            texcoord_b: [0.0; 2],
            bone_id_a: Default::default(),
            bone_id_b: Default::default(),
            weight_a: Default::default(),
            weight_b: Default::default(),
            state: 0,
        }
    }

    fn shift(
        &mut self,
        position: [f32; 3],
        normal: [f32; 3],
        texcoord: Option<[f32; 2]>,
        bone_id: BoneId,
        weight: Weight,
    ) {
        self.position_b = position;
        self.normal_b = normal;
        if let Some(texcoord) = texcoord {
            self.texcoord_b = texcoord;
        }
        self.bone_id_b = bone_id;
        self.weight_b = weight;
    }
}

impl<BoneId, Weight> VertexHandler<BoneId, Weight> for TriangleFan<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    fn handle_vertex(
        &mut self,
        position: [f32; 3],
        normal: [f32; 3],
        texcoord: Option<[f32; 2]>,
        bone_id: BoneId,
        weight: Weight,
    ) {
        match self.state {
            0 => {
                self.position_a = position;
                self.normal_a = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_a = texcoord;
                }
                self.bone_id_a = bone_id;
                self.weight_a = weight;
                self.state = 1;
            }
            1 => {
                self.shift(position, normal, texcoord, bone_id, weight);
                self.state = 2;
            }
            2 => {
                self.positions.push(self.position_a);
                self.positions.push(self.position_b);
                self.positions.push(position);
                self.normals.push(self.normal_a);
                self.normals.push(self.normal_b);
                self.normals.push(normal);
                if let Some(texcoord) = texcoord {
                    self.texcoords.push(self.texcoord_a);
                    self.texcoords.push(self.texcoord_b);
                    self.texcoords.push(texcoord);
                }
                self.bone_ids.push(self.bone_id_a);
                self.bone_ids.push(self.bone_id_b);
                self.bone_ids.push(bone_id);
                self.weights.push(self.weight_a);
                self.weights.push(self.weight_b);
                self.weights.push(weight);
                self.shift(position, normal, texcoord, bone_id, weight);
            }
            _ => unreachable!(),
        }
    }

    fn finish(self) -> Batch<BoneId, Weight> {
        Batch {
            positions: self.positions,
            normals: self.normals,
            texcoords: self.texcoords,
            bone_ids: self.bone_ids,
            weights: self.weights,
        }
    }
}

#[derive(Debug)]
pub struct Batch<BoneId, Weight>
where
    BoneId: VertexAttribute,
    Weight: VertexAttribute,
{
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub bone_ids: Vec<BoneId>,
    pub weights: Vec<Weight>,
}

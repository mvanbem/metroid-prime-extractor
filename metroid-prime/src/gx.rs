use std::io::Read;

use anyhow::{bail, Result};
use gamecube::bytes::ReadFrom;
use gamecube::ReadBytesExt;

#[derive(Debug)]
pub struct DisplayList {
    data: Vec<u8>,
}

impl DisplayList {
    pub fn parse(
        &self,
        vertex_attr_flags: u32,
        position_data: &[u8],
        normal_data: &[u8],
        uv_float_data: &[u8],
    ) -> Result<Vec<Batch>> {
        let mut r = self.data.as_slice();
        let mut batches = Vec::new();
        loop {
            let opcode = r.read_u8()?;
            if opcode == 0 {
                break;
            }
            let primitive_type = opcode & 0xf8;
            let vertex_format = opcode & 0x07;
            assert_eq!(vertex_format, 0x00); // There are others, but they aren't implemented yet.
            match primitive_type {
                0x90 => batches.push(Self::parse_batch(
                    &mut r,
                    Triangles::new(),
                    vertex_attr_flags,
                    position_data,
                    normal_data,
                    uv_float_data,
                )?),
                0x98 => batches.push(Self::parse_batch(
                    &mut r,
                    TriangleStrip::new(),
                    vertex_attr_flags,
                    position_data,
                    normal_data,
                    uv_float_data,
                )?),
                0xa0 => batches.push(Self::parse_batch(
                    &mut r,
                    TriangleFan::new(),
                    vertex_attr_flags,
                    position_data,
                    normal_data,
                    uv_float_data,
                )?),
                _ => bail!("unexpected GX primitive type: 0x{:02x}", primitive_type),
            }
        }
        Ok(batches)
    }

    fn parse_batch<R: Read, H: VertexHandler>(
        r: &mut R,
        mut vertex_handler: H,
        vertex_attr_flags: u32,
        position_data: &[u8],
        normal_data: &[u8],
        uv_float_data: &[u8],
    ) -> Result<Batch> {
        let count = r.read_u16()?;
        for _ in 0..count {
            // Position
            assert!((vertex_attr_flags & 0x3) != 0);
            let position = {
                let index = r.read_u16()?;
                let mut data = &position_data[index as usize * 12..];
                let x = f32::from_bits(data.read_u32()?);
                let y = f32::from_bits(data.read_u32()?);
                let z = f32::from_bits(data.read_u32()?);
                [x, y, z]
            };
            // Normal
            assert!((vertex_attr_flags & 0xc) != 0);
            let normal = {
                let index = r.read_u16()?;
                let mut data = &normal_data[index as usize * 12..];
                let x = f32::from_bits(data.read_u32()?);
                let y = f32::from_bits(data.read_u32()?);
                let z = f32::from_bits(data.read_u32()?);
                [x, y, z]
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
            let uv = if (vertex_attr_flags & 0x300) != 0 {
                // TODO: Look at material to know whether float or short UV coordinates are read.
                let index = r.read_u16()?;
                let mut data = &uv_float_data[index as usize * 8..];
                let s = f32::from_bits(data.read_u32()?);
                let t = f32::from_bits(data.read_u32()?);
                Some([s, t])
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

            vertex_handler.handle_vertex(position, normal, uv);
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

trait VertexHandler {
    fn handle_vertex(&mut self, position: [f32; 3], normal: [f32; 3], texcoord: Option<[f32; 2]>);
    fn finish(self) -> Batch;
}

struct Triangles {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    position_a: [f32; 3],
    position_b: [f32; 3],
    normal_a: [f32; 3],
    normal_b: [f32; 3],
    texcoord_a: [f32; 2],
    texcoord_b: [f32; 2],
    // 0: empty buffer
    // 1: one buffered vertex
    // 2: two buffered vertices
    state: u8,
}

impl Triangles {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Vec::new(),
            position_a: [0.0; 3],
            position_b: [0.0; 3],
            normal_a: [0.0; 3],
            normal_b: [0.0; 3],
            texcoord_a: [0.0; 2],
            texcoord_b: [0.0; 2],
            state: 0,
        }
    }
}

impl VertexHandler for Triangles {
    fn handle_vertex(&mut self, position: [f32; 3], normal: [f32; 3], texcoord: Option<[f32; 2]>) {
        match self.state {
            0 => {
                self.position_a = position;
                self.normal_a = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_a = texcoord;
                }
                self.state = 1;
            }
            1 => {
                self.position_b = position;
                self.normal_b = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_b = texcoord;
                }
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
                self.state = 0;
            }
            _ => unreachable!(),
        }
    }

    fn finish(self) -> Batch {
        assert_eq!(self.state, 0);
        Batch {
            positions: self.positions,
            normals: self.normals,
            texcoords: self.texcoords,
        }
    }
}

struct TriangleStrip {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    position_a: [f32; 3],
    position_b: [f32; 3],
    normal_a: [f32; 3],
    normal_b: [f32; 3],
    texcoord_a: [f32; 2],
    texcoord_b: [f32; 2],
    // 0: empty buffer
    // 1: one buffered vertex
    // 2: two buffered vertices, even parity
    // 3: two buffered vertices, odd parity
    state: u8,
}

impl TriangleStrip {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Vec::new(),
            position_a: [0.0; 3],
            position_b: [0.0; 3],
            normal_a: [0.0; 3],
            normal_b: [0.0; 3],
            texcoord_a: [0.0; 2],
            texcoord_b: [0.0; 2],
            state: 0,
        }
    }

    fn shift(&mut self, position: [f32; 3], normal: [f32; 3], texcoord: Option<[f32; 2]>) {
        self.position_a = self.position_b;
        self.normal_a = self.normal_b;
        self.texcoord_a = self.texcoord_b;
        self.position_b = position;
        self.normal_b = normal;
        if let Some(texcoord) = texcoord {
            self.texcoord_b = texcoord;
        }
    }
}

impl VertexHandler for TriangleStrip {
    fn handle_vertex(&mut self, position: [f32; 3], normal: [f32; 3], texcoord: Option<[f32; 2]>) {
        match self.state {
            0 => {
                self.position_a = position;
                self.normal_a = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_a = texcoord;
                }
                self.state = 1;
            }
            1 => {
                self.position_b = position;
                self.normal_b = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_b = texcoord;
                }
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
                self.shift(position, normal, texcoord);
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
                self.shift(position, normal, texcoord);
                self.state = 2;
            }
            _ => unreachable!(),
        }
    }

    fn finish(self) -> Batch {
        Batch {
            positions: self.positions,
            normals: self.normals,
            texcoords: self.texcoords,
        }
    }
}

struct TriangleFan {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    texcoords: Vec<[f32; 2]>,
    position_a: [f32; 3],
    position_b: [f32; 3],
    normal_a: [f32; 3],
    normal_b: [f32; 3],
    texcoord_a: [f32; 2],
    texcoord_b: [f32; 2],
    // 0: empty buffer
    // 1: one buffered vertex
    // 2: two buffered vertices
    state: u8,
}

impl TriangleFan {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            texcoords: Vec::new(),
            position_a: [0.0; 3],
            position_b: [0.0; 3],
            normal_a: [0.0; 3],
            normal_b: [0.0; 3],
            texcoord_a: [0.0; 2],
            texcoord_b: [0.0; 2],
            state: 0,
        }
    }

    fn shift(&mut self, position: [f32; 3], normal: [f32; 3], texcoord: Option<[f32; 2]>) {
        self.position_b = position;
        self.normal_b = normal;
        if let Some(texcoord) = texcoord {
            self.texcoord_b = texcoord;
        }
    }
}

impl VertexHandler for TriangleFan {
    fn handle_vertex(&mut self, position: [f32; 3], normal: [f32; 3], texcoord: Option<[f32; 2]>) {
        match self.state {
            0 => {
                self.position_a = position;
                self.normal_a = normal;
                if let Some(texcoord) = texcoord {
                    self.texcoord_a = texcoord;
                }
                self.state = 1;
            }
            1 => {
                self.shift(position, normal, texcoord);
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
                self.shift(position, normal, texcoord);
            }
            _ => unreachable!(),
        }

        self.position_b = position;
        self.normal_b = normal;
    }

    fn finish(self) -> Batch {
        Batch {
            positions: self.positions,
            normals: self.normals,
            texcoords: self.texcoords,
        }
    }
}

#[derive(Debug)]
pub struct Batch {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
}

use std::io::{self, Read};

use anyhow::{anyhow, bail, Result};
use arrayvec::ArrayVec;
use byteorder::BigEndian;

pub trait ReadBytesExt: Read {
    fn read_i8(&mut self) -> io::Result<i8>;
    fn read_i16(&mut self) -> io::Result<i16>;
    fn read_i32(&mut self) -> io::Result<i32>;
    fn read_u8(&mut self) -> io::Result<u8>;
    fn read_u16(&mut self) -> io::Result<u16>;
    fn read_u32(&mut self) -> io::Result<u32>;
}

impl<T> ReadBytesExt for T
where
    T: Read,
{
    fn read_i8(&mut self) -> io::Result<i8> {
        <Self as byteorder::ReadBytesExt>::read_i8(self)
    }

    fn read_i16(&mut self) -> io::Result<i16> {
        <Self as byteorder::ReadBytesExt>::read_i16::<BigEndian>(self)
    }

    fn read_i32(&mut self) -> io::Result<i32> {
        <Self as byteorder::ReadBytesExt>::read_i32::<BigEndian>(self)
    }

    fn read_u8(&mut self) -> io::Result<u8> {
        <Self as byteorder::ReadBytesExt>::read_u8(self)
    }

    fn read_u16(&mut self) -> io::Result<u16> {
        <Self as byteorder::ReadBytesExt>::read_u16::<BigEndian>(self)
    }

    fn read_u32(&mut self) -> io::Result<u32> {
        <Self as byteorder::ReadBytesExt>::read_u32::<BigEndian>(self)
    }
}

pub trait ReadFrom {
    fn read_from<R: Read>(r: &mut R) -> Result<Self>
    where
        Self: Sized;
}

impl ReadFrom for u32 {
    fn read_from<R: Read>(r: &mut R) -> Result<Self> {
        Ok(r.read_u32()?)
    }
}

pub trait ReadTypedExt: Read {
    fn read_typed<T: ReadFrom>(&mut self) -> Result<T>;
}

impl<R: Read> ReadTypedExt for R {
    fn read_typed<T: ReadFrom>(&mut self) -> Result<T> {
        Ok(T::read_from(self)?)
    }
}

pub trait ReadFromWithContext {
    type Context;

    fn read_from_with_context<R: Read>(r: &mut R, ctx: Self::Context) -> Result<Self>
    where
        Self: Sized;
}

pub trait ReadTypedWithContextExt: Read {
    fn read_typed_with_context<T: ReadFromWithContext>(&mut self, ctx: T::Context) -> Result<T>;
}

impl<R: Read> ReadTypedWithContextExt for R {
    fn read_typed_with_context<T: ReadFromWithContext>(&mut self, ctx: T::Context) -> Result<T> {
        Ok(T::read_from_with_context(self, ctx)?)
    }
}

pub trait ReadArrayExt: ReadTypedExt {
    fn read_array<T: ReadFrom, const N: usize>(&mut self) -> Result<[T; N]>;
}

impl<R: Read> ReadArrayExt for R {
    fn read_array<T: ReadFrom, const N: usize>(&mut self) -> Result<[T; N]> {
        Ok(std::iter::from_fn(|| Some(self.read_typed()))
            .take(N)
            .collect::<Result<ArrayVec<T, N>>>()
            .map(|v| match v.into_inner() {
                Ok(result) => result,
                Err(_) => panic!(),
            })?)
    }
}

pub trait ReadLengthPrefixedStringExt: Read {
    fn read_length_prefixed_string(&mut self) -> Result<String>;
}

impl<R: Read> ReadLengthPrefixedStringExt for R {
    fn read_length_prefixed_string(&mut self) -> Result<String> {
        let len = self.read_u32()?;
        let mut s = String::with_capacity(len as usize);
        for _ in 0..len {
            let b = self.read_u8()?;
            if !b.is_ascii() {
                bail!("Non-ASCII byte: 0x{:02x}", b)
            }
            s.push(b as char);
        }
        Ok(s)
    }
}

pub trait ReadAsciiCStringExt: Read {
    fn read_ascii_c_string(&mut self) -> Result<String>;
}

impl<R: Read> ReadAsciiCStringExt for R {
    fn read_ascii_c_string(&mut self) -> Result<String> {
        let mut s = String::new();
        loop {
            let b = self.read_u8()?;
            if b == 0 {
                break;
            }
            if !b.is_ascii() {
                bail!("Non-ASCII byte: 0x{:02x}", b)
            }
            s.push(b as char);
        }
        Ok(s)
    }
}

pub trait ReadFixedCapacityAsciiCStringExt: Read {
    fn read_fixed_capacity_ascii_c_string(&mut self, len: usize) -> Result<String>;
}

impl<R: Read> ReadFixedCapacityAsciiCStringExt for R {
    fn read_fixed_capacity_ascii_c_string(&mut self, len: usize) -> Result<String> {
        let mut data = Vec::with_capacity(len);
        while data.len() < len {
            data.push(self.read_u8()?);
        }

        data.into_iter()
            .scan((), |_, b| match b {
                0 => None,
                b if b.is_ascii() => Some(Ok(b as char)),
                _ => Some(Err(anyhow!("Non-ASCII character"))),
            })
            .collect()
    }
}

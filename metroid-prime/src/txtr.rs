use std::io::Write;

use anyhow::{bail, Result};
use gamecube::ReadBytesExt;
use png::{BitDepth, ColorType};

pub fn dump<W: Write>(mut data: &[u8], w: &mut W) -> Result<()> {
    let format = data.read_u32()?;
    let width = data.read_u16()? as usize;
    let height = data.read_u16()? as usize;
    let mip_count = data.read_u32()?;

    match format {
        0x0 => dump_i4(data, width, height, mip_count, w),
        0x1 => dump_i8(data, width, height, mip_count, w),
        0x2 => dump_ia4(data, width, height, mip_count, w),
        0x3 => dump_ia8(data, width, height, mip_count, w),
        0x4 => dump_c4(data, width, height, mip_count, w),
        0x5 => dump_c8(data, width, height, mip_count, w),
        0x7 => dump_rgb565(data, width, height, mip_count, w),
        0x8 => dump_rgb5a3(data, width, height, mip_count, w),
        0x9 => dump_rgba8(data, width, height, mip_count, w),
        0xa => dump_cmpr(data, width, height, mip_count, w),
        _ => bail!("unknown texture format: {}", format),
    }
}

fn decode_rgb5a3(encoded: u16) -> [u8; 4] {
    if encoded & 0x8000 == 0 {
        let extend3 = |x| (x << 5) | (x << 2) | (x >> 1);
        let extend4 = |x| (x << 4) | x;
        [
            extend4(((encoded >> 8) & 0xf) as u8),
            extend4(((encoded >> 4) & 0xf) as u8),
            extend4((encoded & 0xf) as u8),
            extend3((encoded >> 12) as u8),
        ]
    } else {
        let extend5 = |x| (x << 3) | (x >> 2);
        [
            extend5(((encoded >> 10) & 0x1f) as u8),
            extend5(((encoded >> 5) & 0x1f) as u8),
            extend5((encoded & 0x1f) as u8),
            0xff,
        ]
    }
}

fn decode_rgb565(encoded: u16) -> [u8; 4] {
    let extend5 = |x| (x << 3) | (x >> 2);
    let extend6 = |x| (x << 2) | (x >> 4);
    [
        extend5(((encoded >> 11) & 0x1f) as u8),
        extend6(((encoded >> 5) & 0x3f) as u8),
        extend5((encoded & 0x1f) as u8),
        0xff,
    ]
}

fn palette_fetcher(format: u32) -> Result<fn(&[u8], usize) -> Result<[u8; 4]>> {
    match format {
        0x1 => Ok(|data, index| Ok(decode_rgb565((&data[2 * index..]).read_u16()?))),
        0x2 => Ok(|data, index| Ok(decode_rgb5a3((&data[2 * index..]).read_u16()?))),
        _ => bail!("unknown palette format: {}", format),
    }
}

fn dump_i4<W: Write>(
    data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 7) / 8;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 8;
        let fine_y = y % 8;
        for x in 0..width {
            let coarse_x = x / 8;
            let fine_x = x % 8;
            let offset = 32 * (blocks_wide * coarse_y + coarse_x) + (8 * fine_y + fine_x) / 2;
            let x = data[offset];
            let i = if x & 1 == 0 { x >> 4 } else { x & 0xf };
            let i = i << 4 | i;
            decoded.extend_from_slice(&[i, i, i, 255]);
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

fn dump_i8<W: Write>(
    data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 7) / 8;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 4;
        let fine_y = y % 4;
        for x in 0..width {
            let coarse_x = x / 8;
            let fine_x = x % 8;
            let offset = 32 * (blocks_wide * coarse_y + coarse_x) + 1 * (8 * fine_y + fine_x);
            let i = data[offset];
            decoded.extend_from_slice(&[i, i, i, 255]);
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

fn dump_ia4<W: Write>(
    data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 7) / 8;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 4;
        let fine_y = y % 4;
        for x in 0..width {
            let coarse_x = x / 8;
            let fine_x = x % 8;
            let offset = 32 * (blocks_wide * coarse_y + coarse_x) + 1 * (8 * fine_y + fine_x);
            let encoded = data[offset];
            let i = encoded >> 4;
            let i = (i << 4) | i;
            let a = encoded & 0xf;
            let a = (a << 4) | a;
            decoded.extend_from_slice(&[i, i, i, a]);
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

fn dump_ia8<W: Write>(
    data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 3) / 4;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 4;
        let fine_y = y % 4;
        for x in 0..width {
            let coarse_x = x / 4;
            let fine_x = x % 4;
            let offset = 32 * (blocks_wide * coarse_y + coarse_x) + 2 * (4 * fine_y + fine_x);
            let i = data[offset];
            let a = data[offset + 1];
            decoded.extend_from_slice(&[i, i, i, a]);
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

fn dump_c4<W: Write>(
    mut data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let palette_fetcher = palette_fetcher(data.read_u32()?)?;
    assert_eq!(data.read_u16()?, 1);
    assert_eq!(data.read_u16()?, 16);
    let palette = &data[..32];
    let data = &data[32..];

    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 7) / 8;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 8;
        let fine_y = y % 8;
        for x in 0..width {
            let coarse_x = x / 8;
            let fine_x = x % 8;
            let offset = 32 * (blocks_wide * coarse_y + coarse_x) + (8 * fine_y + fine_x) / 2;
            let x = data[offset];
            let c = if x & 1 == 0 { x >> 4 } else { x & 0xf };
            decoded.extend_from_slice(&palette_fetcher(palette, c as usize)?);
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

fn dump_c8<W: Write>(
    mut data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let palette_fetcher = palette_fetcher(data.read_u32()?)?;
    assert_eq!(data.read_u16()?, 256);
    assert_eq!(data.read_u16()?, 1);
    let palette = &data[..512];
    let data = &data[512..];

    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 7) / 8;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 4;
        let fine_y = y % 4;
        for x in 0..width {
            let coarse_x = x / 8;
            let fine_x = x % 8;
            let offset = 32 * (blocks_wide * coarse_y + coarse_x) + 1 * (8 * fine_y + fine_x);
            let c = data[offset];
            decoded.extend_from_slice(&palette_fetcher(palette, c as usize)?);
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

fn dump_rgb565<W: Write>(
    data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 3) / 4;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 4;
        let fine_y = y % 4;
        for x in 0..width {
            let coarse_x = x / 4;
            let fine_x = x % 4;
            let offset = 32 * (blocks_wide * coarse_y + coarse_x) + 2 * (4 * fine_y + fine_x);
            let encoded = (&data[offset..]).read_u16()?;
            decoded.extend_from_slice(&decode_rgb565(encoded));
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

fn dump_rgb5a3<W: Write>(
    data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 3) / 4;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 4;
        let fine_y = y % 4;
        for x in 0..width {
            let coarse_x = x / 4;
            let fine_x = x % 4;
            let offset = 32 * (blocks_wide * coarse_y + coarse_x) + 2 * (4 * fine_y + fine_x);
            let encoded = (&data[offset..]).read_u16()?;
            decoded.extend_from_slice(&decode_rgb5a3(encoded));
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

fn dump_rgba8<W: Write>(
    data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 3) / 4;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 4;
        let fine_y = y % 4;
        for x in 0..width {
            let coarse_x = x / 4;
            let fine_x = x % 4;
            let offset = 64 * (blocks_wide * coarse_y + coarse_x) + 2 * (4 * fine_y + fine_x);
            let a = data[offset];
            let r = data[offset + 1];
            let g = data[offset + 32];
            let b = data[offset + 33];
            decoded.extend_from_slice(&[r, g, b, a]);
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

fn dump_cmpr<W: Write>(
    data: &[u8],
    width: usize,
    height: usize,
    _mip_count: u32,
    w: &mut W,
) -> Result<()> {
    let mut decoded = Vec::with_capacity(width * height * 4);
    let blocks_wide = (width + 7) / 8;
    for y in (0..height).rev() {
        let y = height - y - 1;
        let coarse_y = y / 8;
        for x in 0..width {
            let coarse_x = x / 8;
            let sub_block = ((y / 4) & 1) << 1 | ((x / 4) & 1);
            let dxt1_offset = 32 * (blocks_wide * coarse_y + coarse_x) + 8 * sub_block;
            let mut dxt1_data = &data[dxt1_offset..dxt1_offset + 8];
            let color_a_encoded = dxt1_data.read_u16()?;
            let color_b_encoded = dxt1_data.read_u16()?;
            let color_a = decode_rgb565(color_a_encoded);
            let color_b = decode_rgb565(color_b_encoded);
            let palette = if color_a > color_b {
                [
                    color_a,
                    color_b,
                    [
                        ((2 * color_a[0] as u16 + color_b[0] as u16) / 3) as u8,
                        ((2 * color_a[1] as u16 + color_b[1] as u16) / 3) as u8,
                        ((2 * color_a[2] as u16 + color_b[2] as u16) / 3) as u8,
                        ((2 * color_a[3] as u16 + color_b[3] as u16) / 3) as u8,
                    ],
                    [
                        ((color_a[0] as u16 + 2 * color_b[0] as u16) / 3) as u8,
                        ((color_a[1] as u16 + 2 * color_b[1] as u16) / 3) as u8,
                        ((color_a[2] as u16 + 2 * color_b[2] as u16) / 3) as u8,
                        ((color_a[3] as u16 + 2 * color_b[3] as u16) / 3) as u8,
                    ],
                ]
            } else {
                [
                    color_a,
                    color_b,
                    [
                        ((color_a[0] as u16 + color_b[0] as u16) / 2) as u8,
                        ((color_a[1] as u16 + color_b[1] as u16) / 2) as u8,
                        ((color_a[2] as u16 + color_b[2] as u16) / 2) as u8,
                        ((color_a[3] as u16 + color_b[3] as u16) / 2) as u8,
                    ],
                    [0, 0, 0, 0],
                ]
            };
            let index = (dxt1_data[y % 4] >> (2 * (3 - x % 4))) & 3;
            decoded.extend_from_slice(&palette[index as usize]);
        }
    }

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&decoded)?;
    Ok(())
}

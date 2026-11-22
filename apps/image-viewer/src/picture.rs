use anyhow::{Context, Result, bail, ensure};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, RgbaImage};
use std::{
    fs::File,
    io::{Cursor, Read},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_PIXELS: u64 = 32 * 1024 * 1024;

pub fn open(path: &Path) -> Result<RgbaImage> {
    let file = File::options()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(path)?;
    let meta = file.metadata()?;
    ensure!(meta.is_file(), "Choose a regular image file.");
    ensure!(
        meta.len() <= MAX_FILE,
        "Image exceeds the 64 MiB file limit."
    );
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_FILE,
        "Image exceeds the 64 MiB file limit."
    );
    decode(&bytes)
}

fn decode(bytes: &[u8]) -> Result<RgbaImage> {
    let format = image::guess_format(bytes).context("Choose a PNG or JPEG image.")?;
    let complete = match format {
        ImageFormat::Jpeg => jpeg_complete(&bytes[2..]),
        ImageFormat::Png => png_complete(&bytes[8..]),
        _ => bail!("Choose a PNG or JPEG image."),
    };
    ensure!(complete, "Image is incomplete.");
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    ensure!(
        u64::from(width) * u64::from(height) <= MAX_PIXELS,
        "Image exceeds the 32 megapixel limit."
    );
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}

// Both decoders can return pixels before reaching the end of the stream.
// Follow segment lengths so an embedded Exif thumbnail cannot supply the outer EOI.
fn jpeg_complete(mut bytes: &[u8]) -> bool {
    while let Some(offset) = bytes.iter().position(|&b| b == 0xff) {
        bytes = &bytes[offset + 1..];
        let Some((&marker, rest)) = bytes.split_first() else {
            break;
        };
        match marker {
            0xff => continue,
            0xd9 => return true,
            0x00 | 0x01 | 0xd0..=0xd7 => bytes = rest,
            _ => {
                let Some(len) = rest.first_chunk::<2>() else {
                    break;
                };
                let len = u16::from_be_bytes(*len) as usize;
                if len < 2 || len > rest.len() {
                    break;
                }
                bytes = &rest[len..];
            }
        }
    }
    false
}

fn png_complete(mut bytes: &[u8]) -> bool {
    // 4-byte length, 4-byte type, payload, 4-byte CRC.
    while bytes.len() >= 12 {
        let length = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        if length > bytes.len() - 12 {
            break;
        }
        if &bytes[4..8] == b"IEND" {
            return length == 0 && bytes[8..12] == [0xae, 0x42, 0x60, 0x82];
        }
        bytes = &bytes[12 + length..];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

    const JPEG: &[u8] = include_bytes!("../../../tests/fixtures/North Point.jpg");

    #[test]
    fn rotated_jpeg_has_upright_landmarks() {
        let image = decode(JPEG).unwrap();
        assert_eq!(image.dimensions(), (480, 300));
        for (x, y, expected) in [(111, 92, [255u8, 242, 200]), (450, 260, [32, 63, 64])] {
            assert!(
                image
                    .get_pixel(x, y)
                    .0
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| a.abs_diff(b) < 6)
            );
        }
        assert!(decode(&JPEG[..JPEG.len() - 2]).is_err());
    }

    #[test]
    fn png_preserves_alpha_and_requires_complete_end_chunk() {
        let mut bytes = Vec::new();
        let pixels = [73, 144, 218, 128];
        PngEncoder::new(&mut bytes)
            .write_image(&pixels, 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        assert_eq!(decode(&bytes).unwrap().as_raw(), &pixels);
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
    }
}

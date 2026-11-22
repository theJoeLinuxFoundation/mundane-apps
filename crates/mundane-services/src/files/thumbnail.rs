use image::{ImageFormat, ImageReader, Limits};
use md5::{Digest, Md5};
use std::{
    fs::{self, DirBuilder, File},
    io::{BufReader, Cursor, Read},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

fn cache_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| Path::new(&p).join(".cache")))
        .map(|p| p.join("thumbnails/normal"))
}

/// Read a valid freedesktop preview or generate one for a small, local PNG or JPEG.
pub fn preview(path: &Path) -> Option<Vec<u8>> {
    read_or_generate(path, &cache_dir()?)
}

fn read_or_generate(path: &Path, cache: &Path) -> Option<Vec<u8>> {
    // Open first so cached thumbnails also respect the source's current read permissions.
    let file = File::options()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(path)
        .ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    let uri = url::Url::from_file_path(path).ok()?.to_string();
    let mtime = meta.mtime().to_string();
    let nanos = meta.mtime_nsec().to_string();
    let size = meta.len().to_string();
    let thumb = cache.join(format!("{:x}.png", Md5::digest(uri.as_bytes())));
    if let Ok(cached) = File::open(&thumb) {
        let mut data = Vec::new();
        cached.take(256 * 1024 + 1).read_to_end(&mut data).ok()?;
        if data.len() <= 256 * 1024
            && let Ok(reader) = png::Decoder::new(Cursor::new(&data)).read_info()
        {
            let info = reader.info();
            let tag = |key| {
                info.uncompressed_latin1_text
                    .iter()
                    .find(|v| v.keyword == key)
                    .map(|v| v.text.as_str())
            };
            if info.width <= 128
                && info.height <= 128
                && tag("Thumb::URI") == Some(&uri)
                && tag("Thumb::MTime") == Some(&mtime)
                && tag("Thumb::Size").is_none_or(|v| v == size)
                && tag("Mundane::MTimeNS").is_none_or(|v| v == nanos)
                && image::load_from_memory_with_format(&data, ImageFormat::Png).is_ok()
            {
                return Some(data);
            }
        }
    }
    if meta.len() > 32 * 1024 * 1024 {
        return None;
    }
    let mut reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .ok()?;
    if !matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg)) {
        return None;
    }
    let mut limits = Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let image = reader.decode().ok()?.thumbnail(128, 128).to_rgba8();
    let after = fs::metadata(path).ok()?;
    if (after.dev(), after.ino(), after.len(), after.modified().ok())
        != (meta.dev(), meta.ino(), meta.len(), meta.modified().ok())
    {
        return None;
    }
    let mut data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut data, image.width(), image.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        for (k, v) in [
            ("Thumb::URI", uri),
            ("Thumb::MTime", mtime),
            ("Thumb::Size", size),
            ("Mundane::MTimeNS", nanos),
        ] {
            encoder.add_text_chunk(k.into(), v).ok()?;
        }
        encoder
            .write_header()
            .ok()?
            .write_image_data(image.as_raw())
            .ok()?;
    }
    // A read-only cache still permits a preview for this listing.
    if DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(cache)
        .is_ok()
        && let Ok(mut temp) = tempfile::NamedTempFile::new_in(cache)
    {
        use std::io::Write;
        if temp.write_all(&data).is_ok() {
            let _ = temp.persist(thumb);
        }
    }
    Some(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_tracks_source_and_recovers_from_corruption() {
        let tmp = super::super::tests::fixture();
        let path = tmp.path().join("image.png");
        let cache = tmp.path().join("cache");
        let save = |red| {
            image::RgbaImage::from_pixel(180, 60, image::Rgba([red, 0, 0, 255]))
                .save(&path)
                .unwrap()
        };
        save(150);
        let first = read_or_generate(&path, &cache).unwrap();
        assert_eq!(image::load_from_memory(&first).unwrap().width(), 128);
        assert_eq!(first, read_or_generate(&path, &cache).unwrap());
        let cached = fs::read_dir(&cache)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(&cached, b"incomplete").unwrap();
        assert_eq!(first, read_or_generate(&path, &cache).unwrap());
        save(220);
        assert_ne!(first, read_or_generate(&path, &cache).unwrap());
    }
}

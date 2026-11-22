use anyhow::{Context, Result, ensure};
use pdfium_render::prelude::*;
use std::{
    fs::File,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{Mutex, OnceLock},
};

const MAX_BYTES: u64 = 128 << 20;
const MAX_HITS: usize = 10_000;

pub struct Book {
    doc: Mutex<PdfDocument<'static>>,
    pub pages: i32,
    pub name: String,
}
#[derive(Clone)]
pub struct Hit {
    pub page: i32,
    boxes: Vec<PdfRect>,
}
pub struct Raster {
    pub pixels: image::RgbaImage, // BGRA for GPUI.
    pub logical: (f32, f32),
    pub highlights: Vec<[f32; 4]>, // Fractions of the displayed page, top-left origin.
    pub text: String,
}

fn engine() -> Result<&'static Pdfium> {
    static ENGINE: OnceLock<Result<Pdfium, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            Pdfium::bind_to_system_library()
                .map(Pdfium::new)
                .map_err(|e| format!("Could not load the system PDFium library. {e}"))
        })
        .as_ref()
        .map_err(|s| anyhow::anyhow!("{s}"))
}

impl Book {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::options()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open(path)?;
        let meta = file.metadata()?;
        ensure!(meta.is_file(), "Choose a regular PDF file.");
        ensure!(meta.len() <= MAX_BYTES, "PDF exceeds 128 MiB.");
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        ensure!(bytes.len() as u64 <= MAX_BYTES, "PDF exceeds 128 MiB.");
        let doc = engine()?.load_pdf_from_byte_vec(bytes, None).map_err(|_| {
            anyhow::anyhow!("Cannot read PDF. The file may be damaged or password-protected.")
        })?;
        let pages = doc.pages().len();
        ensure!(pages > 0, "The PDF has no pages.");
        Ok(Self {
            doc: Mutex::new(doc),
            pages,
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
        })
    }

    pub fn search(&self, query: &str) -> Result<Vec<Hit>> {
        ensure!(
            !query.trim().is_empty() && !query.contains('\0'),
            "Enter search text."
        );
        let doc = self.doc.lock().unwrap();
        let mut hits = Vec::new();
        for number in 1..=self.pages {
            let page = doc.pages().get(number - 1)?;
            let text = page.text()?;
            let search = text.search(query, &PdfSearchOptions::new())?;
            while let Some(found) = search.find_next() {
                ensure!(
                    hits.len() < MAX_HITS,
                    "More than 10,000 matches. Narrow the search."
                );
                hits.push(Hit {
                    page: number,
                    boxes: found.iter().map(|segment| segment.bounds()).collect(),
                });
            }
        }
        Ok(hits)
    }

    pub fn render(
        &self,
        number: i32,
        zoom: f32,
        display_scale: f32,
        hit: Option<&Hit>,
    ) -> Result<Raster> {
        ensure!(
            (1..=self.pages).contains(&number),
            "Page must be between 1 and {}.",
            self.pages
        );
        let doc = self.doc.lock().unwrap();
        let page = doc.pages().get(number - 1)?;
        let (w, h) = (page.width().value, page.height().value);
        ensure!(
            w.is_finite() && h.is_finite() && w > 0. && h > 0. && w.max(h) <= 20_000.,
            "Unsupported page dimensions."
        );
        let logical = (w * zoom, h * zoom);
        let scale = (zoom * display_scale)
            .min(4096. / w.max(h))
            .min((12_000_000. / (w * h)).sqrt());
        let config = PdfRenderConfig::new()
            .set_target_width((w * scale).round().max(1.) as i32)
            .set_target_height((h * scale).round().max(1.) as i32)
            .set_reverse_byte_order(false);
        let bitmap = page.render_with_config(&config)?;
        let (width, height) = (bitmap.width() as u32, bitmap.height() as u32);
        let mut pixels = image::RgbaImage::from_raw(width, height, bitmap.as_raw_bytes())
            .context("Invalid PDF bitmap stride.")?;
        let mut highlights = Vec::new();
        if let Some(hit) = hit.filter(|hit| hit.page == number) {
            for rect in &hit.boxes {
                let (x1, y1) = page.points_to_pixels(rect.left(), rect.bottom(), &config)?;
                let (x2, y2) = page.points_to_pixels(rect.right(), rect.top(), &config)?;
                // Multiply the paper colour so dark text remains legible under the mark.
                for y in y1.min(y2).max(0)..y1.max(y2).min(height as i32) {
                    for x in x1.min(x2).max(0)..x1.max(x2).min(width as i32) {
                        let bgra = &mut pixels.get_pixel_mut(x as u32, y as u32).0;
                        bgra[0] /= 3;
                        bgra[1] = (u16::from(bgra[1]) * 222 / 255) as u8;
                    }
                }
                highlights.push([
                    x1.min(x2) as f32 / width as f32,
                    y1.min(y2) as f32 / height as f32,
                    x1.max(x2) as f32 / width as f32,
                    y1.max(y2) as f32 / height as f32,
                ]);
            }
        }
        Ok(Raster {
            pixels,
            logical,
            highlights,
            text: page.text()?.all(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_bounds_search_render_and_bad_input() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let doc = Book::open(&root.join("../../tests/fixtures/Coastal survey.pdf")).unwrap();
        assert_eq!(doc.pages, 2);
        assert!(doc.render(0, 1., 1., None).is_err() && doc.render(3, 1., 1., None).is_err());
        assert!(Book::open(&root.join("Cargo.toml")).is_err());
        assert!(doc.search("mountain").unwrap().is_empty());
        let hits = doc.search("CHANNEL").unwrap();
        assert_eq!(hits.iter().map(|h| h.page).collect::<Vec<_>>(), [1, 2]);
        let raster = doc.render(2, 1., 2., Some(&hits[1])).unwrap();
        assert_eq!(raster.pixels.dimensions(), (840, 560));
        assert!(raster.text.contains("high water"));
        let [x, y, right, bottom] = raster.highlights[0];
        assert!(x > 0.12 && right < 0.4 && y > 0.30 && bottom < 0.4 && x < right && y < bottom);
    }
}

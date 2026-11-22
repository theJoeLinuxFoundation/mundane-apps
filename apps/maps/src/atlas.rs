use crate::{
    geo::{Camera, Size, Tile},
    source::{Area, MAX_BYTES, MAX_TILES, Place, Source},
    vector::{self, Label},
};
use anyhow::{Context, Result, ensure};
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tiny_skia::{Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform};
#[derive(Clone)]
pub struct Viewport {
    pub camera: Camera,
    pub size: Size,
    pub place: Option<Place>,
    pub offline: Option<Arc<Area>>,
    pub style: vector::Style,
}
pub struct Scene {
    pub image: Arc<RenderImage>,
    pub labels: Vec<Label>,
    pub missing: usize,
    pub message: Option<String>,
    pub area: Option<Arc<Area>>,
    pub range: (u8, u8),
    pub camera: Camera,
}
#[derive(Default)]
pub struct Atlas {
    cache: VecDeque<(Tile, Option<Arc<[u8]>>)>,
    source_key: String,
}
impl Atlas {
    pub fn clear(&mut self) {
        self.cache.clear()
    }
    fn remember(&mut self, id: Tile, body: Option<Arc<[u8]>>) {
        self.cache.push_back((id, body));
        let mut bytes = self
            .cache
            .iter()
            .filter_map(|(_, b)| b.as_ref())
            .map(|b| b.len())
            .sum::<usize>();
        while self.cache.len() > 128 || bytes > MAX_BYTES {
            let (_, old) = self.cache.pop_front().unwrap();
            bytes -= old.map_or(0, |b| b.len())
        }
    }
    pub fn render(
        &mut self,
        src: &Source,
        v: &Viewport,
        cancelled: impl Fn() -> bool,
    ) -> Result<Scene> {
        let mut c = v.camera;
        c.validate()?;
        let size = v.size;
        let ratio = size.ratio();
        ensure!(
            size.width > 0 && size.height > 0 && ratio > 0.,
            "Missing viewport"
        );
        let manifest = if v.offline.is_none() {
            Some(src.manifest()?)
        } else {
            None
        };
        let range = if let Some(m) = &manifest {
            let key = m.identity();
            if self.source_key != key {
                self.clear();
                self.source_key = key
            }
            (m.minzoom, m.maxzoom)
        } else {
            (c.zoom, c.zoom)
        };
        c.zoom = c.zoom.clamp(range.0, range.1);
        let placements = c.tiles(size);
        ensure!(
            placements.len() <= MAX_TILES,
            "Viewport exceeds the tile limit"
        );
        let ratio = ratio
            .min(4096. / size.width as f32)
            .min(3072. / size.height as f32);
        let mut image = Pixmap::new(
            (size.width as f32 * ratio).round().max(1.) as u32,
            (size.height as f32 * ratio).round().max(1.) as u32,
        )
        .context("Invalid map dimensions")?;
        image.fill(v.style.ground());
        let tile_size = (256. * ratio).round().max(1.) as u32;
        let mut bytes = 0;
        let mut entries = HashMap::new();
        let mut labels: Vec<Label> = vec![];
        let mut missing = 0;
        let mut message = None;
        for p in placements {
            ensure!(!cancelled(), "View changed");
            let body = if let Some(a) = &v.offline {
                a.tiles
                    .iter()
                    .find(|(id, _)| *id == p.tile)
                    .map(|(_, b)| Arc::<[u8]>::from(b.as_slice()))
            } else if let Some((_, data)) = self.cache.iter().find(|(id, _)| *id == p.tile) {
                data.clone()
            } else if message.is_none() {
                match src.tile(manifest.as_ref().unwrap(), p.tile, MAX_BYTES - bytes) {
                    Ok(data) => {
                        let body = data.map(Arc::<[u8]>::from);
                        self.remember(p.tile, body.clone());
                        body
                    }
                    Err(e) => {
                        message = Some(e.to_string());
                        None
                    }
                }
            } else {
                None
            };
            let rendered = if let Some(b) = body {
                if let std::collections::hash_map::Entry::Vacant(entry) = entries.entry(p.tile) {
                    bytes += b.len();
                    ensure!(bytes <= MAX_BYTES, "View exceeds 50 MB");
                    entry.insert(b.clone());
                }
                match vector::paint(&b, tile_size, v.style) {
                    Ok(tile) => Some(tile),
                    Err(e) => {
                        message = Some(format!("Unreadable vector tile · {e}"));
                        None
                    }
                }
            } else {
                None
            };
            let (tile, annotations) = rendered.unwrap_or_else(|| {
                missing += 1;
                (missing_tile(tile_size, v.style), vec![])
            });
            image.draw_pixmap(
                (p.left * ratio as f64).round() as i32,
                (p.top * ratio as f64).round() as i32,
                tile.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            for mut a in annotations {
                a.x += p.left as f32;
                a.y += p.top as f32;
                if a.x >= 0.
                    && a.y >= 0.
                    && a.x + 70. < size.width as f32
                    && a.y + 18. < size.height as f32
                    && labels.len() < 28
                    && !labels
                        .iter()
                        .any(|old| (old.x - a.x).abs() < 95. && (old.y - a.y).abs() < 25.)
                {
                    labels.push(a)
                }
            }
        }
        let area = if missing == 0 && v.offline.is_none() {
            Some(Arc::new(Area {
                camera: c,
                place: v.place.clone(),
                tiles: entries
                    .into_iter()
                    .map(|(id, b)| (id, b.to_vec().into()))
                    .collect(),
            }))
        } else {
            None
        };
        let (w, h) = (image.width(), image.height());
        let mut bgra = image.take();
        for p in bgra.as_chunks_mut::<4>().0 {
            p.swap(0, 2)
        }
        Ok(Scene {
            camera: c,
            range,
            image: Arc::new(RenderImage::new([Frame::new(
                RgbaImage::from_raw(w, h, bgra).unwrap(),
            )])),
            labels,
            missing,
            message,
            area,
        })
    }
}
fn missing_tile(size: u32, style: vector::Style) -> Pixmap {
    let mut tile = Pixmap::new(size, size).unwrap();
    tile.fill(style.color(0x2f3131, 0xe1e3e2, 0x101010, 0xffffff));
    let mut path = PathBuilder::new();
    for i in -8..8 {
        let x = i as f32 * size as f32 / 8.;
        path.move_to(x, 0.);
        path.line_to(x + size as f32, size as f32)
    }
    let mut paint = Paint::default();
    paint.set_color(style.color(0x595d5b, 0x858b88, 0xffffff, 0));
    tile.stroke_path(
        &path.finish().unwrap(),
        &paint,
        &Stroke::default(),
        Transform::identity(),
        None,
    );
    tile
}

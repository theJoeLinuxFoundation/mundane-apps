//! Decode the geometry and naming fields of Mapbox Vector Tile v2 (OpenMapTiles schema).
use anyhow::{Context, Result, bail, ensure};
use prost::Message;
use std::io::Read;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

#[derive(Message)]
struct VectorTile {
    #[prost(message, repeated, tag = "3")]
    layers: Vec<Layer>,
}
#[derive(Message)]
struct Layer {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(message, repeated, tag = "2")]
    features: Vec<Feature>,
    #[prost(string, repeated, tag = "3")]
    keys: Vec<String>,
    #[prost(message, repeated, tag = "4")]
    values: Vec<Value>,
    #[prost(uint32, optional, tag = "5")]
    extent: Option<u32>,
}
#[derive(Message)]
struct Value {
    #[prost(string, optional, tag = "1")]
    text: Option<String>,
}
#[derive(Message)]
struct Feature {
    #[prost(uint32, repeated, packed = "true", tag = "2")]
    tags: Vec<u32>,
    #[prost(uint32, tag = "3")]
    kind: u32,
    #[prost(uint32, repeated, packed = "true", tag = "4")]
    geometry: Vec<u32>,
}
impl Feature {
    fn tag<'a>(&self, key: &str, layer: &'a Layer) -> Option<&'a str> {
        self.tags.as_chunks::<2>().0.iter().find_map(|p| {
            (layer.keys.get(p[0] as usize)?.as_str() == key)
                .then(|| layer.values.get(p[1] as usize)?.text.as_deref())
                .flatten()
        })
    }
}
fn color(rgb: u32) -> Color {
    Color::from_rgba8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Dark,
    Light,
    DarkContrast,
    LightContrast,
}

impl From<mundane_ui::Theme> for Style {
    fn from(theme: mundane_ui::Theme) -> Self {
        match (theme.light, theme.high_contrast) {
            (false, false) => Self::Dark,
            (true, false) => Self::Light,
            (false, true) => Self::DarkContrast,
            (true, true) => Self::LightContrast,
        }
    }
}
impl Style {
    pub fn color(self, dark: u32, light: u32, dark_hc: u32, light_hc: u32) -> Color {
        color(match self {
            Self::Dark => dark,
            Self::Light => light,
            Self::DarkContrast => dark_hc,
            Self::LightContrast => light_hc,
        })
    }
    pub fn ground(self) -> Color {
        self.color(0x383e36, 0xe9eee5, 0x101010, 0xffffff)
    }
}
fn order(layer: &str) -> u8 {
    match layer {
        "landcover" | "landuse" | "park" => 1,
        "water" => 2,
        "building" => 3,
        "waterway" => 4,
        "boundary" => 5,
        "transportation" => 6,
        "place" | "poi" | "water_name" | "transportation_name" => 7,
        _ => 0,
    }
}
fn style(name: &str, class: &str, palette: Style) -> (Color, f32) {
    let (dark, light, dark_hc, light_hc, width) = match name {
        "water" | "waterway" => (0x28536b, 0x9dcde5, 0x005172, 0x5db0da, 1.5),
        "park" | "landcover" => (0x3d5945, 0xb4d5af, 0x203518, 0xb4e1a0, 0.),
        "landuse" if matches!(class, "residential" | "commercial" | "industrial") => {
            (0x4b4940, 0xe0dbce, 0x242424, 0xe4e0d7, 0.)
        }
        "landuse" => (0x455341, 0xc8d8be, 0x26391e, 0xcbdcb3, 0.),
        "building" => (0x686359, 0xcbbfae, 0x656565, 0x9c958b, 0.),
        "boundary" => (0xb59ab6, 0x805885, 0xe7aaff, 0x64206f, 1.),
        _ if matches!(class, "motorway" | "trunk" | "primary") => {
            (0xe4bf80, 0x99631a, 0xffdc76, 0x693800, 3.)
        }
        _ if matches!(class, "secondary" | "tertiary") => {
            (0xc6bca4, 0x937f62, 0xffedcc, 0x53412d, 2.3)
        }
        _ => (0x9d9f8d, 0x858b7e, 0xffffff, 0x303030, 1.3),
    };
    (palette.color(dark, light, dark_hc, light_hc), width)
}
#[derive(Clone)]
pub struct Label {
    pub x: f32,
    pub y: f32,
    pub text: String,
}
fn geometry(g: &[u32], scale: f32) -> Result<(PathBuilder, Option<(f32, f32)>)> {
    let mut path = PathBuilder::new();
    let mut anchor = None;
    let mut words = g.iter().copied();
    let (mut x, mut y) = (0i32, 0i32);
    while let Some(c) = words.next() {
        let n = (c >> 3) as usize;
        ensure!(n > 0, "Empty geometry command");
        match c & 7 {
            op @ (1 | 2) => {
                ensure!(n <= words.len() / 2, "Incomplete vector geometry");
                for _ in 0..n {
                    let delta = |v: u32| (v >> 1) as i32 ^ -((v & 1) as i32);
                    x = x
                        .checked_add(delta(words.next().unwrap()))
                        .context("Invalid tile coordinate")?;
                    y = y
                        .checked_add(delta(words.next().unwrap()))
                        .context("Invalid tile coordinate")?;
                    let p = (x as f32 * scale, y as f32 * scale);
                    if op == 1 {
                        path.move_to(p.0, p.1);
                        anchor.get_or_insert(p);
                    } else {
                        path.line_to(p.0, p.1)
                    }
                }
            }
            7 if n == 1 => path.close(),
            _ => bail!("Unsupported vector geometry command"),
        }
    }
    Ok((path, anchor))
}
pub fn paint(bytes: &[u8], pixels: u32, palette: Style) -> Result<(Pixmap, Vec<Label>)> {
    let mut unzipped = vec![];
    let data = if bytes.starts_with(&[31, 139]) {
        flate2::read::GzDecoder::new(bytes)
            .take(super::source::MAX_TILE as u64 + 1)
            .read_to_end(&mut unzipped)?;
        ensure!(
            unzipped.len() <= super::source::MAX_TILE,
            "Expanded tile exceeds limit"
        );
        unzipped.as_slice()
    } else {
        bytes
    };
    let mut vector = VectorTile::decode(data)?;
    vector.layers.sort_by_key(|l| order(&l.name));
    let mut img = Pixmap::new(pixels, pixels).context("Invalid tile dimensions")?;
    img.fill(palette.ground());
    let mut labels = vec![];
    let ratio = pixels as f32 / 256.;
    for layer in &vector.layers {
        let rank = order(&layer.name);
        if rank == 0 {
            continue;
        }
        let extent = layer.extent.unwrap_or(4096);
        ensure!(extent > 0, "Zero tile extent");
        for f in &layer.features {
            let (path, anchor) = geometry(&f.geometry, 256. / extent as f32)?;
            if rank == 7 {
                if let Some((x, y)) = anchor
                    && (0. ..256.).contains(&x)
                    && (0. ..256.).contains(&y)
                    && labels.len() < 64
                    && let Some(name) = f.tag("name", layer).filter(|s| !s.is_empty())
                {
                    labels.push(Label {
                        x,
                        y,
                        text: name.chars().take(80).collect(),
                    })
                }
                continue;
            }
            let Some(path) = path.finish() else { continue };
            let (col, width) = style(
                &layer.name,
                f.tag("class", layer).unwrap_or_default(),
                palette,
            );
            let mut ink = Paint::default();
            ink.set_color(col);
            let tf = Transform::from_scale(ratio, ratio);
            match f.kind {
                3 => img.fill_path(&path, &ink, FillRule::EvenOdd, tf, None),
                2 if width > 0. => img.stroke_path(
                    &path,
                    &ink,
                    &Stroke {
                        width,
                        ..Default::default()
                    },
                    tf,
                    None,
                ),
                _ => {}
            }
        }
    }
    Ok((img, labels))
}

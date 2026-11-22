use crate::geo::{Camera, Tile};
use anyhow::{Context, Result, ensure};
use mundane_services::provider as net;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{collections::HashSet, fs, path::PathBuf, time::Duration};
use url::Url;
pub const TILES: &str = "https://tiles.openfreemap.org/planet";
pub const SEARCH: &str = "https://nominatim.openstreetmap.org/search";
pub const MAX_TILES: usize = 3000;
pub const MAX_BYTES: usize = 50_000_000;
pub const MAX_TILE: usize = 4_000_000;
#[derive(Clone)]
pub struct Source {
    pub data: PathBuf,
    pub dir: PathBuf,
    pub tiles: String,
    pub search: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Place {
    pub display_name: String,
    pub lat: String,
    pub lon: String,
}
impl Place {
    pub fn point(&self) -> Option<(f64, f64)> {
        let c = Camera {
            lat: self.lat.parse().ok()?,
            lon: self.lon.parse().ok()?,
            zoom: 0,
        };
        c.validate().ok()?;
        Some((c.lat, c.lon))
    }
    pub fn name(&self) -> &str {
        self.display_name
            .split(',')
            .next()
            .unwrap_or(&self.display_name)
    }
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Manifest {
    tiles: Vec<String>,
    #[serde(default)]
    pub minzoom: u8,
    #[serde(default = "maxzoom")]
    pub maxzoom: u8,
    #[serde(default)]
    scheme: String,
}
fn maxzoom() -> u8 {
    14
}
impl Manifest {
    pub fn identity(&self) -> String {
        format!("{:?} {}", self.tiles, self.scheme)
    }
    fn url(&self, base: &str, t: Tile) -> Result<Url> {
        let template = self.tiles.first().context("TileJSON has no tile URLs")?;
        ensure!(
            ["{z}", "{x}", "{y}"].iter().all(|p| template.contains(p)),
            "Expected an XYZ tile template"
        );
        let y = if self.scheme == "tms" {
            (1 << t.z) - 1 - t.y
        } else {
            t.y
        };
        net::endpoint(
            net::endpoint(base)?
                .join(
                    &template
                        .replace("{z}", &t.z.to_string())
                        .replace("{x}", &t.x.to_string())
                        .replace("{y}", &y.to_string()),
                )?
                .as_str(),
        )
    }
}
#[derive(Serialize, Deserialize)]
pub struct Area {
    pub camera: Camera,
    pub place: Option<Place>,
    pub tiles: Vec<(Tile, ByteBuf)>,
}
impl Area {
    pub fn check(&self) -> Result<()> {
        self.camera.validate()?;
        ensure!(
            self.place.as_ref().is_none_or(|p| p.point().is_some()),
            "Invalid saved place"
        );
        let mut seen = HashSet::new();
        ensure!(
            !self.tiles.is_empty() && self.tiles.len() <= MAX_TILES,
            "Area exceeds tile limit or has no tiles"
        );
        let mut total = 0;
        for (id, body) in &self.tiles {
            ensure!(
                id.valid()
                    && id.z == self.camera.zoom
                    && seen.insert(*id)
                    && body.len() <= MAX_TILE,
                "Invalid saved tile"
            );
            total += body.len();
            ensure!(total <= MAX_BYTES, "Area exceeds 50 MB")
        }
        Ok(())
    }
}
impl Source {
    pub fn favourites(&self) -> Result<Vec<Place>> {
        let path = self.data.join("favourites.json");
        if path.try_exists()? {
            net::read(&path, 1024 * 1024)
        } else {
            Ok(vec![])
        }
    }
    pub fn toggle_favourite(&self, place: &Place) -> Result<bool> {
        ensure!(place.point().is_some(), "Invalid place coordinates");
        let _lock = net::lock(&self.data.join("places.lock"))?;
        let mut list = self.favourites()?;
        let added = if let Some(i) = list.iter().position(|p| p.point() == place.point()) {
            list.remove(i);
            false
        } else {
            ensure!(list.len() < 512, "The favourites list is full.");
            list.push(place.clone());
            true
        };
        net::store(&self.data.join("favourites.json"), &list)?;
        Ok(added)
    }

    fn area_path(&self) -> PathBuf {
        self.dir.join(format!("{}.area", net::key(&self.tiles)))
    }
    pub fn has_area(&self) -> bool {
        self.area_path().is_file()
    }
    pub fn save(&self, a: &Area) -> Result<()> {
        a.check()?;
        let b = rmp_serde::to_vec(a)?;
        ensure!(b.len() <= MAX_BYTES, "Area exceeds 50 MB");
        net::save(&self.area_path(), &b)
    }
    pub fn open(&self) -> Result<Area> {
        use std::io::Read;
        let mut bytes = vec![];
        fs::File::open(self.area_path())?
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= MAX_BYTES, "Area exceeds 50 MB");
        let a: Area = rmp_serde::from_slice(&bytes)?;
        a.check()?;
        Ok(a)
    }
    pub fn places(&self, q: &str, offline: bool) -> Result<Vec<Place>> {
        let query = q.split_whitespace().collect::<Vec<_>>().join(" ");
        ensure!(
            !query.is_empty() && query.len() <= 512,
            "Enter a place name of up to 512 bytes"
        );
        let mut url = net::endpoint(&self.search)?;
        url.query_pairs_mut()
            .append_pair("q", &query)
            .append_pair("format", "jsonv2")
            .append_pair("limit", "6");
        let file = self.dir.join("places").join(net::key(&format!(
            "{}\n{}",
            self.search,
            query.to_lowercase()
        )));
        let _guard = net::lock(&file.with_extension("lock"))?;
        if let Ok(cached) = net::read::<Vec<Place>>(&file, 512_000) {
            return Ok(cached);
        }
        ensure!(!offline, "No cached results for this search");
        let r = net::get(
            &url,
            &net::agent("maps"),
            None,
            512_000,
            &self.dir.join("search-rate"),
            Duration::from_millis(1200),
        )?;
        ensure!(r.status == 200, "Place search returned HTTP {}", r.status);
        let mut items: Vec<Place> = serde_json::from_slice(&r.body)?;
        items.retain(|p| !p.display_name.is_empty() && p.point().is_some());
        items.truncate(6);
        net::store(&file, &items)?;
        Ok(items)
    }
    pub fn manifest(&self) -> Result<Manifest> {
        let endpoint = net::endpoint(&self.tiles)?;
        let file = self.dir.join("sources").join(net::key(&self.tiles));
        let _guard = net::lock(&file.with_extension("lock"))?;
        let old = net::read::<(u64, Manifest)>(&file, 2_000_000).ok();
        if let Some((expiry, m)) = &old
            && *expiry > net::now()
        {
            return Ok(m.clone());
        }
        let update = (|| {
            let r = net::get(
                &endpoint,
                &net::agent("maps"),
                None,
                2_000_000,
                &self.dir.join("tile-rate"),
                Duration::ZERO,
            )?;
            ensure!(r.status == 200, "TileJSON returned HTTP {}", r.status);
            let m: Manifest = serde_json::from_slice(&r.body)?;
            ensure!(
                m.minzoom <= m.maxzoom && m.maxzoom <= 22,
                "Unsupported TileJSON zoom range"
            );
            m.url(&self.tiles, Tile { x: 0, y: 0, z: 0 })?;
            net::store(
                &file,
                &(r.expires.unwrap_or_else(|| net::now() + 86400), &m),
            )?;
            Ok(m)
        })();
        update.or_else(|e| old.map(|(_, m)| m).ok_or(e))
    }
    pub fn tile(&self, m: &Manifest, id: Tile, remaining: usize) -> Result<Option<Vec<u8>>> {
        let r = net::get(
            &m.url(&self.tiles, id)?,
            &net::agent("maps"),
            None,
            remaining.min(MAX_TILE),
            &self.dir.join("tile-rate"),
            Duration::ZERO,
        )?;
        match r.status {
            200 | 204 => Ok(Some(r.body)),
            404 | 410 => Ok(None),
            code => anyhow::bail!("Tile server returned HTTP {code}"),
        }
    }
}

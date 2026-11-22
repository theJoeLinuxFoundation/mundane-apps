use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
pub const TILE: f64 = 256.;
pub const EDGE: f64 = 85.0511287798066;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Camera {
    pub lat: f64,
    pub lon: f64,
    pub zoom: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tile {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}
impl Tile {
    pub fn valid(self) -> bool {
        self.z <= 22 && self.x < (1 << self.z) && self.y < (1 << self.z)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
    pub scale: u16,
}
impl Size {
    pub fn ratio(self) -> f32 {
        self.scale as f32 / 100.
    }
}
pub struct Placement {
    pub tile: Tile,
    pub left: f64,
    pub top: f64,
}
impl Camera {
    pub fn validate(self) -> Result<()> {
        ensure!(
            self.zoom <= 22
                && (-90. ..=90.).contains(&self.lat)
                && (-180. ..=180.).contains(&self.lon),
            "Invalid map coordinates"
        );
        Ok(())
    }
    fn world(self) -> (f64, f64) {
        let x = (self.lon + 180.) / 360.;
        let y = (1.
            - self.lat.clamp(-EDGE, EDGE).to_radians().tan().asinh() / std::f64::consts::PI)
            / 2.;
        (x.rem_euclid(1.), y.clamp(0., 1. - f64::EPSILON))
    }
    fn span(self) -> f64 {
        TILE * (1u32 << self.zoom) as f64
    }
    pub fn pan(&mut self, dx: f64, dy: f64) {
        let (x, y) = self.world();
        self.lon = (x + dx / self.span()).rem_euclid(1.) * 360. - 180.;
        let y = (y + dy / self.span()).clamp(0., 1.);
        self.lat = (std::f64::consts::PI * (1. - 2. * y))
            .sinh()
            .atan()
            .to_degrees()
    }
    pub fn position(self, lat: f64, lon: f64, size: Size) -> (f32, f32) {
        let (x, y) = self.world();
        let (a, b) = Self { lat, lon, ..self }.world();
        (
            ((a - x + 0.5).rem_euclid(1.) * self.span() - self.span() * 0.5
                + size.width as f64 / 2.) as f32,
            ((b - y) * self.span() + size.height as f64 / 2.) as f32,
        )
    }
    pub fn tiles(self, size: Size) -> Vec<Placement> {
        let n = 1i64 << self.zoom;
        let (x, y) = self.world();
        let left = x * self.span() - size.width as f64 / 2.;
        let top = y * self.span() - size.height as f64 / 2.;
        let mut tiles = vec![];
        for j in ((top / TILE).floor() as i64).max(0)
            ..(((top + size.height as f64) / TILE).ceil() as i64).min(n)
        {
            for i in (left / TILE).floor() as i64..((left + size.width as f64) / TILE).ceil() as i64
            {
                tiles.push(Placement {
                    tile: Tile {
                        x: i.rem_euclid(n) as u32,
                        y: j as u32,
                        z: self.zoom,
                    },
                    left: i as f64 * TILE - left,
                    top: j as f64 * TILE - top,
                })
            }
        }
        tiles
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapping_poles_and_exact_tile_selection() {
        let mut c = Camera {
            lat: 0.,
            lon: 0.,
            zoom: 1,
        };
        let size = Size {
            width: 256,
            height: 256,
            scale: 100,
        };
        assert_eq!(c.tiles(size).len(), 4);
        c.lon = 179.;
        c.pan(8., 0.);
        assert!(c.lon < -170.);
        assert!(c.tiles(size).iter().any(|p| p.tile.x == 1));
        c.pan(0., -1e12);
        assert!((c.lat - EDGE).abs() < 1e-10);
        assert!(c.tiles(size).iter().all(|p| p.tile.y == 0));
        assert!(Camera { lat: f64::NAN, ..c }.validate().is_err());
    }
}

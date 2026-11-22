use crate::provider::{self as net, read, store};
use anyhow::{Context, Result, bail, ensure};
use jiff::{Timestamp, tz::TimeZone};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, time::Duration};

pub const ENDPOINT: &str = "https://api.met.no/weatherapi/locationforecast/2.0/compact";
const LIMIT: usize = 4 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub zone: String,
}
impl Location {
    pub fn custom(text: &str) -> Result<Self> {
        let mut parts = text.split_whitespace();
        let lat = parts
            .next()
            .context("Enter latitude longitude and IANA time zone")?
            .trim_end_matches(',')
            .parse::<f64>()?;
        let lon = parts.next().context("Enter longitude")?.parse::<f64>()?;
        let zone = parts.next().context("Enter an IANA time zone")?;
        ensure!(
            parts.next().is_none(),
            "Enter latitude longitude and time zone"
        );
        let l = Self {
            name: format!("{lat:.4}, {lon:.4}"),
            lat,
            lon,
            zone: zone.into(),
        };
        l.validate()?;
        Ok(l)
    }
    fn validate(&self) -> Result<()> {
        ensure!(
            (-90. ..=90.).contains(&self.lat) && (-180. ..=180.).contains(&self.lon),
            "Coordinates out of range"
        );
        TimeZone::get(&self.zone)?;
        Ok(())
    }
}
fn coordinate(s: &str, deg: usize) -> Option<f64> {
    let (degrees, rest) = s.get(1..)?.split_at_checked(deg)?;
    let min = rest.get(..2)?.parse::<u32>().ok()?;
    let sec = if rest.len() == 4 {
        rest[2..].parse().ok()?
    } else {
        0
    };
    let value = degrees.parse::<f64>().ok()? + min as f64 / 60. + sec as f64 / 3600.;
    Some(if s.starts_with('-') { -value } else { value })
}
pub fn cities() -> Vec<Location> {
    let table = std::fs::read_to_string("/usr/share/zoneinfo/zone.tab").unwrap_or_default();
    let mut out = vec![];
    for line in table.lines().filter(|s| !s.starts_with('#')) {
        let mut cols = line.split('\t');
        let _ = cols.next();
        let (Some(coords), Some(zone)) = (cols.next(), cols.next()) else {
            continue;
        };
        let Some(pos) = coords
            .get(1..)
            .and_then(|s| s.find(['+', '-']))
            .map(|p| p + 1)
        else {
            continue;
        };
        if let (Some(lat), Some(lon)) =
            (coordinate(&coords[..pos], 2), coordinate(&coords[pos..], 3))
        {
            out.push(Location {
                name: zone.rsplit('/').next().unwrap_or(zone).replace('_', " "),
                zone: zone.into(),
                lat,
                lon,
            })
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}
#[derive(Deserialize, Serialize, Clone)]
pub struct Hour {
    pub time: i64,
    pub temperature: f64,
    pub wind: Option<f64>,
    pub condition: String,
}
#[derive(Deserialize, Serialize, Clone)]
pub struct Forecast {
    pub updated: i64,
    pub hours: Vec<Hour>,
}
#[derive(Deserialize, Serialize)]
struct Cached {
    forecast: Forecast,
    expires: u64,
    modified: Option<String>,
}
#[derive(Deserialize)]
struct Document {
    properties: Properties,
}
#[derive(Deserialize)]
struct Properties {
    meta: Meta,
    timeseries: Vec<Step>,
}
#[derive(Deserialize)]
struct Meta {
    updated_at: String,
    units: HashMap<String, String>,
}
#[derive(Deserialize)]
struct Step {
    time: String,
    data: Data,
}
#[derive(Deserialize)]
struct Data {
    instant: InstantData,
    next_1_hours: Option<Period>,
    next_6_hours: Option<Period>,
    next_12_hours: Option<Period>,
}
#[derive(Deserialize)]
struct InstantData {
    details: Details,
}
#[derive(Deserialize)]
struct Details {
    air_temperature: f64,
    wind_speed: Option<f64>,
}
#[derive(Deserialize)]
struct Period {
    summary: Summary,
}
#[derive(Deserialize)]
struct Summary {
    symbol_code: String,
}
fn parse(bytes: &[u8]) -> Result<Forecast> {
    let p = serde_json::from_slice::<Document>(bytes)?.properties;
    ensure!(
        p.meta.units.get("air_temperature").map(String::as_str) == Some("celsius"),
        "Unsupported forecast temperature unit"
    );
    let mut hours = vec![];
    for s in p.timeseries {
        let d = s.data.instant.details;
        if d.wind_speed.is_some() {
            ensure!(
                p.meta.units.get("wind_speed").map(String::as_str) == Some("m/s"),
                "Unsupported wind unit"
            )
        }
        let code = s
            .data
            .next_1_hours
            .or(s.data.next_6_hours)
            .or(s.data.next_12_hours)
            .map(|p| p.summary.symbol_code)
            .unwrap_or_default();
        let condition = code
            .split('_')
            .next()
            .unwrap_or("")
            .replace("partlycloudy", "Partly cloudy")
            .replace("clearsky", "Clear sky")
            .replace("fair", "Fair")
            .replace("lightrain", "Light rain")
            .replace("heavyrain", "Heavy rain")
            .replace("showers", " showers")
            .replace("andthunder", " and thunder");
        hours.push(Hour {
            time: s.time.parse::<Timestamp>()?.as_second(),
            temperature: d.air_temperature,
            wind: d.wind_speed,
            condition,
        })
    }
    ensure!(!hours.is_empty(), "Empty forecast");
    hours.sort_by_key(|h| h.time);
    Ok(Forecast {
        updated: p.meta.updated_at.parse::<Timestamp>()?.as_second(),
        hours,
    })
}
pub fn local(time: i64, zone: &str) -> Result<String> {
    Ok(Timestamp::from_second(time)?
        .to_zoned(TimeZone::get(zone)?)
        .strftime("%a %d %b %H:%M %Z")
        .to_string())
}
#[derive(Clone)]
pub struct Weather {
    pub dir: PathBuf,
    pub endpoint: String,
}
pub struct Report {
    pub forecast: Forecast,
    pub status: String,
}
impl Weather {
    pub fn selected(&self) -> Option<Location> {
        read::<Location>(&self.dir.join("location.json"), 8192)
            .ok()
            .filter(|l| l.validate().is_ok())
    }
    pub fn select(&self, l: &Location) -> Result<()> {
        l.validate()?;
        store(&self.dir.join("location.json"), l)
    }
    pub fn fetch(&self, l: &Location) -> Result<Report> {
        l.validate()?;
        let mut url = net::endpoint(&self.endpoint)?;
        let coord = |n: f64| format!("{:.4}", (n * 10000.).trunc() / 10000.);
        url.query_pairs_mut()
            .append_pair("lat", &coord(l.lat))
            .append_pair("lon", &coord(l.lon));
        let path = self.dir.join(format!("{}.json", net::key(url.as_str())));
        let _guard = net::lock(&path.with_extension("lock"))?;
        let mut old = read::<Cached>(&path, (LIMIT * 2) as u64).ok();
        if old.as_ref().is_some_and(|e| e.expires > net::now()) {
            return Ok(Report {
                forecast: old.unwrap().forecast,
                status: "Cached forecast · current".into(),
            });
        }
        let update = (|| {
            let r = net::get(
                &url,
                &net::agent("weather"),
                old.as_ref().and_then(|e| e.modified.as_deref()),
                LIMIT,
                &self.dir.join("met-requests"),
                Duration::from_secs(2),
            )?;
            let expires = r.expires.unwrap_or_else(|| net::now() + 3600);
            let status = match r.status {
                200 | 203 => {
                    let entry = Cached {
                        forecast: parse(&r.body)?,
                        modified: r.modified,
                        expires,
                    };
                    store(&path, &entry)?;
                    old = Some(entry);
                    if r.status == 203 {
                        "Forecast service version deprecated"
                    } else {
                        "Latest forecast"
                    }
                }
                304 => {
                    let c = old.as_mut().context("304 without a cached forecast")?;
                    c.expires = expires;
                    if r.modified.is_some() {
                        c.modified = r.modified
                    }
                    store(&path, c)?;
                    "Cached forecast · checked"
                }
                code => bail!("Forecast service returned HTTP {code}"),
            };
            Ok::<_, anyhow::Error>(status)
        })();
        let status = match update {
            Ok(s) => s.into(),
            Err(e)
                if old.is_some()
                    && e.downcast_ref::<curl::Error>().is_some_and(|e| {
                        e.is_couldnt_connect()
                            || e.is_couldnt_resolve_host()
                            || e.is_operation_timedout()
                    }) =>
            {
                "Offline · stale cached forecast".into()
            }
            Err(_) if old.is_some() => "Update unavailable · stale cached forecast".into(),
            Err(e) => return Err(e),
        };
        Ok(Report {
            forecast: old.context("Missing forecast")?.forecast,
            status,
        })
    }
}

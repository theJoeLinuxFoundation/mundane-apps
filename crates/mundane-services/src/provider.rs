//! Bounded HTTP reads, atomic cache files and request pacing shared across app instances.
use anyhow::{Context, Result, bail, ensure};
use curl::easy::{Easy, List};
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::Url;

pub const PROJECT: &str = "https://github.com/theJoeLinuxFoundation/mundane-apps";
pub fn agent(app: &str) -> String {
    format!("mundane-{app}/{} {PROJECT}", env!("CARGO_PKG_VERSION"))
}
pub fn cache_dir(app: &str) -> Result<PathBuf> {
    let root = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .context("No cache directory available")?;
    Ok(root.join(format!("mundane-{app}")))
}
pub fn data_dir(app: &str) -> Result<PathBuf> {
    let root = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .context("No user data directory available")?;
    Ok(root.join(format!("mundane-{app}")))
}
pub fn key(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}
pub fn lock(path: &Path) -> Result<File> {
    fs::create_dir_all(path.parent().context("Missing parent directory")?)?;
    let f = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    f.lock()?;
    Ok(f)
}
pub fn save(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().context("Missing cache directory")?;
    fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.persist(path)?;
    Ok(())
}
pub fn store<T: Serialize>(path: &Path, v: &T) -> Result<()> {
    save(path, &serde_json::to_vec(v)?)
}
pub fn read<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    let mut bytes = vec![];
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "Cache entry too large");
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn date(s: &str) -> Option<u64> {
    httpdate::parse_http_date(s)
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}
pub fn endpoint(s: &str) -> Result<Url> {
    let u = Url::parse(s)?;
    ensure!(
        matches!(u.scheme(), "http" | "https")
            && u.has_host()
            && u.username().is_empty()
            && u.password().is_none(),
        "Expected an HTTP(S) endpoint without credentials"
    );
    Ok(u)
}
pub struct Response {
    pub status: u32,
    pub body: Vec<u8>,
    pub expires: Option<u64>,
    pub modified: Option<String>,
    pub retry: Option<u64>,
    location: Option<String>,
}
fn once(url: &Url, ua: &str, modified: Option<&str>, limit: usize) -> Result<Response> {
    let mut h = Easy::new();
    h.url(url.as_str())?;
    h.useragent(ua)?;
    h.accept_encoding("gzip, deflate")?;
    h.timeout(Duration::from_secs(12))?;
    h.connect_timeout(Duration::from_secs(4))?;
    if let Some(m) = modified.filter(|m| date(m).is_some() && !m.contains(['\r', '\n'])) {
        let mut headers = List::new();
        headers.append(&format!("If-Modified-Since: {m}"))?;
        h.http_headers(headers)?
    }
    let mut result = Response {
        status: 0,
        body: vec![],
        expires: None,
        modified: None,
        retry: None,
        location: None,
    };
    {
        let mut t = h.transfer();
        t.write_function(|b| {
            if b.len() > limit.saturating_sub(result.body.len()) {
                return Ok(0);
            }
            result.body.extend_from_slice(b);
            Ok(b.len())
        })?;
        t.header_function(|b| {
            if let Ok(line) = std::str::from_utf8(b)
                && let Some((name, v)) = line.split_once(':')
            {
                let v = v.trim();
                match name.to_ascii_lowercase().as_str() {
                    "expires" => result.expires = date(v),
                    "last-modified" if date(v).is_some() => result.modified = Some(v.into()),
                    "location" => result.location = Some(v.into()),
                    "retry-after" => {
                        result.retry = v
                            .parse::<u64>()
                            .ok()
                            .map(|n| now().saturating_add(n))
                            .or_else(|| date(v))
                    }
                    _ => {}
                }
            }
            true
        })?;
        t.perform()?
    }
    result.status = h.response_code()?;
    Ok(result)
}
/// Each hop shares a per-user gate with other processes. Cache readers do not consume a slot.
pub fn get(
    url: &Url,
    ua: &str,
    modified: Option<&str>,
    limit: usize,
    gate: &Path,
    interval: Duration,
) -> Result<Response> {
    let mut target = url.clone();
    for _ in 0..5 {
        let _guard = lock(&gate.with_extension("lock"))?;
        let state = read::<(u64, u64)>(gate, 256).unwrap_or_default();
        ensure!(
            state.1 <= now(),
            "Provider requested a pause before retrying"
        );
        let gap = interval.as_millis() as u64;
        std::thread::sleep(Duration::from_millis(
            state
                .0
                .saturating_add(gap)
                .saturating_sub(millis())
                .min(gap),
        ));
        store(gate, &(millis(), 0u64))?;
        let r = once(&target, ua, modified, limit)?;
        if r.status == 429 || r.status == 503 {
            store(gate, &(millis(), r.retry.unwrap_or_else(|| now() + 60)))?
        }
        if matches!(r.status, 301 | 302 | 303 | 307 | 308) {
            let redirected =
                target.join(r.location.as_deref().context("Redirect without Location")?)?;
            target = endpoint(redirected.as_str())?;
            ensure!(
                url.scheme() != "https" || target.scheme() == "https",
                "Refused an insecure redirect"
            );
            continue;
        }
        return Ok(r);
    }
    bail!("Too many redirects")
}

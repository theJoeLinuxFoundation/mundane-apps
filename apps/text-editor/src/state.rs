//! Private recovery copies and a shared, most-recent-first list of local file URIs.
use crate::document::{Document, Version};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tempfile::NamedTempFile;
use url::Url;

pub fn directory() -> io::Result<PathBuf> {
    let dir = dirs::state_dir()
        .ok_or_else(|| io::Error::other("No state directory"))?
        .join("mundane-text-editor");
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    Ok(dir)
}
fn atomic(dir: &Path, path: &Path, value: &impl Serialize) -> io::Result<()> {
    let mut file = NamedTempFile::new_in(dir)?;
    serde_json::to_writer(&mut file, value)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    File::open(dir)?.sync_all()
}
fn uri(path: &Path) -> io::Result<String> {
    Url::from_file_path(std::path::absolute(path)?)
        .map(String::from)
        .map_err(|_| io::ErrorKind::InvalidInput.into())
}
pub fn recent(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let f = match File::open(dir.join("recent.json")) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    let uris: Vec<String> = serde_json::from_reader(f)?;
    Ok(uris
        .iter()
        .filter_map(|s| Url::parse(s).ok()?.to_file_path().ok())
        .collect())
}
pub fn remember(dir: &Path, path: &Path) -> io::Result<()> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("recent.lock"))?;
    lock.lock()?;
    let mut list = recent(dir)?
        .iter()
        .map(|p| uri(p))
        .collect::<io::Result<Vec<_>>>()?;
    let new = uri(path)?;
    list.retain(|p| *p != new);
    list.insert(0, new);
    list.truncate(20);
    atomic(dir, &dir.join("recent.json"), &list)
}
#[derive(Serialize, Deserialize)]
pub struct Draft {
    path: Option<String>,
    saved: String,
    version: Option<Version>,
    bom: bool,
    pub text: String,
}
impl Draft {
    pub fn new(doc: &Document, text: &str) -> io::Result<Self> {
        Ok(Self {
            path: doc.path.as_deref().map(uri).transpose()?,
            saved: doc.saved.to_string(),
            version: doc.version.clone(),
            bom: doc.bom,
            text: text.into(),
        })
    }
    pub fn into_document(self) -> io::Result<(Document, String)> {
        let path = self
            .path
            .map(|s| {
                Url::parse(&s)
                    .ok()
                    .and_then(|u| u.to_file_path().ok())
                    .ok_or(io::ErrorKind::InvalidData)
            })
            .transpose()?;
        Ok((
            Document {
                path,
                saved: self.saved.into(),
                version: self.version,
                bom: self.bom,
            },
            self.text,
        ))
    }
}
pub struct Slot {
    lock: File,
    path: PathBuf,
    revision: AtomicU64,
    writer: Mutex<()>,
}
impl Slot {
    fn from_lock(lock: File, path: PathBuf) -> Self {
        Self {
            lock,
            path,
            revision: AtomicU64::new(0),
            writer: Mutex::new(()),
        }
    }
    pub fn create(dir: &Path) -> io::Result<Self> {
        let f = tempfile::Builder::new()
            .prefix("draft-")
            .suffix(".lock")
            .tempfile_in(dir)?;
        f.as_file().lock()?;
        let (lock, path) = f.keep()?;
        Ok(Self::from_lock(lock, path))
    }
    pub fn next(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst) + 1
    }
    pub fn write(&self, rev: u64, draft: &Draft) -> io::Result<()> {
        let _guard = self.writer.lock().unwrap();
        if self.revision.load(Ordering::SeqCst) == rev {
            atomic(
                self.path.parent().unwrap(),
                &self.path.with_extension("json"),
                draft,
            )?;
        }
        Ok(())
    }
    pub fn clear(&self) -> io::Result<()> {
        self.next();
        let _guard = self.writer.lock().unwrap();
        match fs::remove_file(self.path.with_extension("json")) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            r => r,
        }
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        if !self.path.with_extension("json").exists() {
            let _ = fs::remove_file(&self.path);
        }
        let _ = self.lock.unlock();
    }
}
pub fn recover(dir: &Path) -> io::Result<Vec<(Slot, Draft)>> {
    let mut found = vec![];
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "json")
            || !path
                .file_name()
                .unwrap()
                .as_encoded_bytes()
                .starts_with(b"draft-")
        {
            continue;
        }
        let lp = path.with_extension("lock");
        let lock = match File::open(&lp) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        match lock.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => continue,
            Err(std::fs::TryLockError::Error(e)) => return Err(e),
        }
        let f = match File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        found.push((Slot::from_lock(lock, lp), serde_json::from_reader(f)?));
    }
    Ok(found)
}

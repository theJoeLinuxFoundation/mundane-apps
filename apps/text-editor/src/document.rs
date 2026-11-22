//! UTF-8 file I/O. Keep the BOM and original line endings outside of display conventions.
use std::{
    fs::{self, File, Metadata, OpenOptions, Permissions},
    io::{self, ErrorKind, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    device: u64,
    inode: u64,
    change_time: (i64, i64),
}

fn version(info: &Metadata) -> io::Result<Version> {
    if !info.is_file() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "Choose a regular file.",
        ));
    }
    Ok(Version {
        device: info.dev(),
        inode: info.ino(),
        change_time: (info.ctime(), info.ctime_nsec()),
    })
}

#[derive(Clone, Default)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub saved: gpui::SharedString,
    pub bom: bool,
    pub version: Option<Version>,
}

impl Document {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let mut file = open_regular(&path, false)?;
        let before = version(&file.metadata()?)?;
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        if before != version(&file.metadata()?)? {
            return Err(changed());
        }
        let bom = text.starts_with('\u{feff}');
        if bom {
            text.drain(..'\u{feff}'.len_utf8());
        }
        Ok(Self {
            path: Some(path),
            saved: text.into(),
            bom,
            version: Some(before),
        })
    }

    pub fn save(&self, path: PathBuf, text: gpui::SharedString) -> io::Result<Self> {
        // Resolve symlinks to existing documents so saving preserves the link itself.
        let (target, existing) = match fs::canonicalize(&path) {
            Ok(target) => {
                let file = open_regular(&target, true)?;
                let info = file.metadata()?;
                (target, Some((version(&info)?, info.mode())))
            }
            Err(e) if e.kind() == ErrorKind::NotFound && fs::symlink_metadata(&path).is_err() => {
                (path.clone(), None)
            }
            Err(e) => return Err(e),
        };
        let observed = existing.as_ref().map(|(v, _)| v);
        if self.path.as_ref() == Some(&path) && observed != self.version.as_ref() {
            return Err(changed());
        }
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut stage = tempfile::NamedTempFile::new_in(parent)?;
        if self.bom {
            stage.write_all(b"\xef\xbb\xbf")?;
        }
        stage.write_all(text.as_bytes())?;
        if let Some((_, mode)) = &existing {
            stage
                .as_file()
                .set_permissions(Permissions::from_mode(mode & 0o777))?;
        }
        stage.as_file().sync_all()?;
        // Serialize the check and rename for concurrent saves by suite instances.
        let directory = File::open(parent)?;
        directory.lock()?;
        let file = if let Some((expected, _)) = existing {
            if fs::canonicalize(&path)? != target || version(&fs::metadata(&target)?)? != expected {
                return Err(changed());
            }
            stage.persist(&target)?
        } else {
            stage.persist_noclobber(&target)?
        };
        Ok(Self {
            path: Some(path),
            saved: text,
            bom: self.bom,
            version: Some(version(&file.metadata()?)?),
        })
    }
}

fn open_regular(path: &Path, writable: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(!writable)
        .write(writable)
        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
        .open(path)?;
    version(&file.metadata()?)?;
    Ok(file)
}

fn changed() -> io::Error {
    io::Error::other("The file changed on disk. Use Save As to keep your edits in another file.")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")).unwrap()
    }
    #[test]
    fn round_trip_preserves_bytes_symlinks_and_permissions() {
        let dir = fixture();
        let target = dir.path().join("notes");
        let link = dir.path().join("link");
        let raw = "\u{feff}日誌\r\nCafe\u{301} 🙂\r\nlast\n";
        fs::write(&target, raw).unwrap();
        fs::set_permissions(&target, Permissions::from_mode(0o640)).unwrap();
        std::os::unix::fs::symlink("notes", &link).unwrap();
        let doc = Document::open(link.clone()).unwrap();
        doc.save(link.clone(), doc.saved.clone()).unwrap();
        assert!(link.is_symlink());
        assert_eq!(fs::read(&target).unwrap(), raw.as_bytes());
        assert_eq!(fs::metadata(target).unwrap().mode() & 0o777, 0o640);
    }
    #[test]
    fn failed_save_preserves_original_and_rejects_invalid_utf8() {
        let dir = fixture();
        let path = dir.path().join("notes");
        fs::write(&path, "original").unwrap();
        let doc = Document::open(path.clone()).unwrap();
        fs::set_permissions(dir.path(), Permissions::from_mode(0o500)).unwrap();
        let result = doc.save(path.clone(), "replacement".into());
        fs::set_permissions(dir.path(), Permissions::from_mode(0o700)).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        fs::write(&path, [0xff]).unwrap();
        assert!(doc.save(path.clone(), "stale".into()).is_err());
        assert!(Document::open(path.clone()).is_err());
        assert_eq!(fs::read(path).unwrap(), [0xff]);
    }
}
